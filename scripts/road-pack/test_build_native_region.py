"""Run with python3 -m unittest discover -s scripts/road-pack."""
from copy import deepcopy
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import build
import build_native_region as native

FIXTURE = Path(__file__).parent / 'fixtures' / 'shared-road-fixture'
SOURCE, COVERAGE, POLICY = (FIXTURE / name for name in ('survey.osm', 'coverage.json', 'policy.json'))


class SharedRoadProjectionTests(unittest.TestCase):
    def setUp(self):
        self.policy = native.json_document(POLICY)

    def project(self):
        return native.project(SOURCE, COVERAGE, self.policy)

    def test_same_source_supplies_crossings_and_reusable_directed_regional_graph(self):
        asset = self.project()
        self.assertEqual(build.encoded(asset), (FIXTURE / 'native-expected.json').read_bytes())
        with tempfile.TemporaryDirectory() as temp:
            out = Path(temp) / 'crossing'
            manifest = build.generate(SOURCE, COVERAGE, self.policy['bounds'],
                                      self.policy['revision'], self.policy['origin'], out)
            self.assertEqual(manifest['source']['file_sha256'], self.policy['source_sha256'])
            self.assertEqual(manifest['source']['coverage_sha256'], self.policy['coverage_sha256'])
            observed = {}
            for cell in manifest['cells'].values():
                for way in native.json_document(out / cell['file'])['ways']:
                    observed[way['id']] = way
        # Existing v1 crossing product is broader and unchanged. It keeps roads
        # such as the gate, restriction and bridge as observations only.
        self.assertEqual(set(observed), {str(i) for i in range(101, 110)})
        self.assertNotIn('oneway', observed['101']['tags'])
        edges = {edge['id']: edge for edge in asset['edges']}
        self.assertEqual(set(edges), {
            'osm:101:0:forward', 'osm:101:1:forward', 'osm:101:2:forward',
            'osm:102:0:forward', 'osm:102:0:reverse', 'osm:103:0:reverse',
        })
        for ident, edge in edges.items():
            _, wid, i, direction = ident.split(':')
            i = int(i)
            points = observed[wid]['coordinates'][i:i+2]
            refs = observed[wid]['node_ids'][i:i+2]
            if direction == 'reverse':
                points, refs = points[::-1], refs[::-1]
            self.assertEqual(edge['geometry'], [native.point_dto(p) for p in points])
            self.assertEqual((edge['from'], edge['to']), tuple('osm:' + ref for ref in refs))
        self.assertEqual(edges['osm:101:0:forward']['tram_access'], 'unknown')
        self.assertEqual(edges['osm:103:0:reverse']['tram_access'], 'forbidden')
        self.assertEqual(edges['osm:102:0:reverse']['tram_access'], 'allowed')
        # Identical coordinates are not a topology relation.
        self.assertEqual(edges['osm:101:1:forward']['geometry'][1],
                         edges['osm:103:0:reverse']['geometry'][1])
        self.assertNotEqual(edges['osm:101:1:forward']['to'], edges['osm:103:0:reverse']['to'])
        moves = {(t['mode'], t['incoming'], t['outgoing']) for t in asset['transitions']}
        self.assertEqual(moves, {
            ('bus', 'osm:101:0:forward', 'osm:101:1:forward'),
            ('bus', 'osm:101:1:forward', 'osm:101:2:forward'),
            ('bus', 'osm:101:2:forward', 'osm:102:0:forward'),
        })
        self.assertEqual(len(asset['bus_stop_positions']), 3)  # reused for different routes

    def test_junction_turn_requires_positive_policy_and_cannot_widen_mode_or_direction(self):
        saved = self.policy['turns'][0]
        self.policy['turns'] = []
        self.assertEqual(len(self.project()['transitions']), 2)
        for change in (
            {'mode': 'tram'},  # unknown allowedness on the first street
            {'outgoing': 'osm:102:0:reverse'},  # wrong endpoint/direction
            {'outgoing': 'osm:104:0:forward'},  # cannot override gate
            {'outgoing': 'osm:107:0:forward'},  # cannot override restriction
        ):
            with self.subTest(change=change):
                self.policy['turns'] = [dict(saved, **change)]
                with self.assertRaisesRegex(ValueError, 'reviewed movement'):
                    self.project()

    def test_original_direction_required_and_mode_permission_not_inferred(self):
        for removed in (b'<tag k="oneway" v="yes"/>', b'<tag k="bus" v="yes"/>'):
            with self.subTest(removed=removed), tempfile.TemporaryDirectory() as temp:
                raw = Path(temp) / 'survey.osm'
                raw.write_bytes(SOURCE.read_bytes().replace(removed, b'', 1))
                policy = dict(self.policy, source_sha256=build.sha(raw.read_bytes()))
                with self.assertRaisesRegex(ValueError, 'reviewed movement'):
                    native.project(raw, COVERAGE, policy)

    def test_unexpected_revision_or_hidden_source_is_not_usable(self):
        for before, after, error in (
            (b'  <node id="1"', b'  <meta osm_base="2026-11-15T00:00:00Z"/>\n  <node id="1"', 'revision mismatch'),
            (b'<node id="1"', b'<node id="1" visible="false"', 'hidden entity'),
        ):
            with self.subTest(error=error), tempfile.TemporaryDirectory() as temp:
                raw = Path(temp) / 'survey.osm'
                raw.write_bytes(SOURCE.read_bytes().replace(before, after, 1))
                policy = dict(self.policy, source_sha256=build.sha(raw.read_bytes()))
                with self.assertRaisesRegex(ValueError, error):
                    native.project(raw, COVERAGE, policy)

    def test_stop_bounds_and_source_identity_fail_closed(self):
        original = deepcopy(self.policy)
        for change, error in (
            ({'source_sha256': '0'*64}, 'source digest'),
            ({'coverage_sha256': '0'*64}, 'coverage digest'),
            ({'bounds': [138.96, 34.96, 139.02, 35.02]}, 'wholly inside'),
            ({'bus_stop_positions': [{'edge_id': 'osm:102:0:forward', 'position': True}]}, 'boarding location'),
            ({'bus_stop_positions': [{'edge_id': 'osm:102:0:forward', 'position': 1.1}]}, 'boarding location'),
            ({'bus_stop_positions': [{'edge_id': 'osm:101:0:reverse', 'position': 0.5}]}, 'boarding location'),
        ):
            with self.subTest(change=change):
                self.policy = dict(original, **change)
                with self.assertRaisesRegex(ValueError, error):
                    self.project()

    def test_cli_checks_before_writing_and_does_not_replace_an_existing_asset(self):
        with tempfile.TemporaryDirectory() as temp:
            result = Path(temp) / 'asset.json'
            cmd = [sys.executable, str(Path(native.__file__)), str(SOURCE), '--coverage',
                   str(COVERAGE), '--policy', str(POLICY), '--output', str(result)]
            process = subprocess.run(cmd, capture_output=True, check=True)
            self.assertIn(b'"candidate_only": true', process.stdout)
            expected = (FIXTURE / 'native-expected.json').read_bytes()
            self.assertEqual(result.read_bytes(), expected)
            process = subprocess.run(cmd, capture_output=True)
            self.assertEqual(process.returncode, 2)
            self.assertEqual(result.read_bytes(), expected)
            self.assertEqual(list(Path(temp).iterdir()), [result])


if __name__ == '__main__':
    unittest.main()
