"""Run with python3 -m unittest discover -s scripts/road-pack."""
from copy import deepcopy
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET

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

    def test_player_bus_stop_flag_is_independent_and_default_bytes_are_identical(self):
        default = self.project()
        self.policy[native.PLAYER_BUS_STOPS] = False
        self.assertEqual(build.encoded(self.project()), build.encoded(default))
        self.policy[native.PLAYER_BUS_STOPS] = True
        self.assertEqual(self.project(), {**default, native.PLAYER_BUS_STOPS: True})
        for malformed in (None, 0, 1, 'true', []):
            with self.subTest(malformed=malformed):
                self.policy[native.PLAYER_BUS_STOPS] = malformed
                with self.assertRaisesRegex(ValueError, 'must be boolean'):
                    self.project()

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


class SimulatedJunctionTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.source = self.root / 'source.osm'
        self.certificate = self.root / 'junctions.json'
        self.policy = native.json_document(POLICY)
        self.policy.update(turns=[], bus_stop_positions=[], bounds=[138.99, 34.99, 139.02, 35.02])
        self.xml = ET.Element('osm', version='0.6')
        for ident, point in {
            '1': (138.999, 35.), '2': (139., 35.), '3': (139.001, 35.),
            '4': (139., 35.001), '5': (139.005, 35.004), '6': (139.006, 35.004),
        }.items():
            ET.SubElement(self.xml, 'node', id=ident, lon=str(point[0]), lat=str(point[1]))
        self.way('201', ('1', '2', '3'), 'yes')
        self.way('202', ('2', '4'), 'no')
        self.way('210', ('5', '6'), 'yes')  # unaffected positive control row

    def way(self, ident, refs, oneway):
        way = ET.SubElement(self.xml, 'way', id=ident)
        for ref in refs:
            ET.SubElement(way, 'nd', ref=ref)
        for k, v in (('highway', 'residential'), ('oneway', oneway), ('bus', 'yes'), ('tram', 'designated')):
            ET.SubElement(way, 'tag', k=k, v=v)
        return way

    def put_certificate(self, **changes):
        value = {k: self.policy[k] for k in ('source_sha256', 'coverage_sha256', 'revision', 'bounds')}
        value.update(format=1, incident_ways_complete=True, restriction_relations_complete=True)
        value.update(changes)
        self.certificate.write_bytes(build.encoded(value))
        self.policy[native.JUNCTION_CERTIFICATE] = build.sha(self.certificate.read_bytes())

    def sync(self):
        self.source.write_bytes(ET.tostring(self.xml, encoding='utf-8'))
        self.policy['source_sha256'] = build.sha(self.source.read_bytes())
        self.put_certificate()

    def project(self):
        return native.project(self.source, COVERAGE, self.policy, junction_coverage=self.certificate)

    def move_ids(self):
        return {(r['mode'], r['incoming'], r['outgoing']) for r in self.project()['transitions']}

    def test_simple_three_arm_parity_with_explicitly_reviewed_transitions(self):
        self.sync()
        expected = {(mode, a, b) for mode in ('bus', 'tram') for a, b in (
            ('osm:201:0:forward', 'osm:201:1:forward'),
            ('osm:201:0:forward', 'osm:202:0:forward'),
            ('osm:202:0:reverse', 'osm:201:1:forward'),
        )}
        self.assertEqual(self.move_ids(), expected)
        opt_in = self.project()
        self.assertTrue(opt_in['origin'].endswith(native.SIMULATED_TURN_ORIGIN))
        strict = {k: v for k, v in self.policy.items() if k != native.JUNCTION_CERTIFICATE}
        self.assertEqual(native.project(self.source, COVERAGE, strict)['transitions'], [])
        strict['turns'] = opt_in['transitions']
        self.assertEqual(native.project(self.source, COVERAGE, strict),
                         dict(opt_in, origin=strict['origin']))
        self.policy['turns'] = strict['turns']
        self.assertEqual(self.project(), opt_in)  # can opt in without deleting reviewed rows
        self.policy['turns'] = []
        self.xml[:] = list(reversed(self.xml[:]))  # arbitrary raw row order, not order of nd within a way
        self.sync()
        self.assertEqual(build.encoded(self.project()), build.encoded(opt_in))

    def test_certificate_requires_both_pinned_inputs_and_complete_exact_snapshot(self):
        self.sync()
        with self.assertRaisesRegex(ValueError, 'both policy opt-in'):
            native.project(self.source, COVERAGE, self.policy)
        strict = {k: v for k, v in self.policy.items() if k != native.JUNCTION_CERTIFICATE}
        with self.assertRaisesRegex(ValueError, 'both policy opt-in'):
            native.project(self.source, COVERAGE, strict, junction_coverage=self.certificate)
        for change, error in (
            ({'incident_ways_complete': False}, 'affirmative'),
            ({'restriction_relations_complete': 1}, 'affirmative'),
            ({'revision': '2020-01-01T00:00:00Z'}, 'exact region snapshot'),
            ({'source_sha256': '0'*64}, 'exact region snapshot'),
            ({'bounds': self.policy['bounds'][:-1] + [35.9]}, 'exact region snapshot'),
        ):
            with self.subTest(change=change):
                self.put_certificate(**change)
                with self.assertRaisesRegex(ValueError, error):
                    self.project()
        self.put_certificate()
        self.certificate.write_bytes(self.certificate.read_bytes() + b' ')
        with self.assertRaisesRegex(ValueError, 'certificate digest mismatch'):
            self.project()

    def test_unknown_permission_only_refuses_its_mode_across_all_arms(self):
        way = self.xml.find('./way[@id="202"]')
        assert way is not None
        way.remove(way.find('./tag[@k="bus"]'))
        self.sync()
        self.assertEqual({mode for mode, _, _ in self.move_ids()}, {'tram'})
        self.policy['modes'] = ['bus']
        self.assertEqual(self.move_ids(), set())

    def test_affected_restriction_barrier_grade_and_conditional_access_fail_closed(self):
        source = ET.tostring(self.xml)
        for entity, key, value in (
            ('node', 'barrier', 'gate'), ('way', 'bridge', 'yes'),
            ('way', 'layer', '1'), ('way', 'incline', 'up'),
            ('way', 'bus:conditional', 'yes @ (Mo-Fr)'),
        ):
            with self.subTest(key=key):
                self.xml = ET.fromstring(source)
                el = self.xml.find('./node[@id="4"]' if entity == 'node' else './way[@id="202"]')
                ET.SubElement(el, 'tag', k=key, v=value)
                self.sync()
                self.assertEqual(self.move_ids(), set())
        self.xml = ET.fromstring(source)
        relation = ET.SubElement(self.xml, 'relation', id='501')
        for ty, ref, role in (('way', '201', 'from'), ('node', '2', 'via'), ('way', '202', 'to')):
            ET.SubElement(relation, 'member', type=ty, ref=ref, role=role)
        ET.SubElement(relation, 'tag', k='type', v='restriction')
        ET.SubElement(relation, 'tag', k='restriction:bus:conditional', v='no_right_turn @ (Mo-Fr)')
        self.sync()
        self.assertEqual(self.move_ids(), set())
        relation.find('./member[@role="via"]').set('ref', '999')
        self.sync()
        with self.assertRaisesRegex(ValueError, 'unresolved restriction member'):
            self.project()

    def test_unmapped_or_complex_arm_and_coincident_node_cannot_join(self):
        source = ET.tostring(self.xml)
        for kind in ('boundary', 'quantity', 'parallel', 'disconnected'):
            with self.subTest(kind=kind):
                self.xml = ET.fromstring(source)
                if kind == 'boundary':
                    self.xml.find('./node[@id="4"]').set('lat', str(self.policy['bounds'][3]))
                elif kind == 'quantity':
                    ET.SubElement(self.xml, 'node', id='7', lon='139.', lat='34.999')
                    self.way('203', ('2', '7'), 'no')
                elif kind == 'parallel':
                    self.xml.find('./node[@id="4"]').attrib.update(lon='139.002', lat='35.')
                else:
                    ET.SubElement(self.xml, 'node', id='7', lon='139.', lat='35.')
                    self.xml.find('./way[@id="202"]/nd').set('ref', '7')
                self.sync()
                relevant = {(m, a, b) for m, a, b in self.move_ids()
                            if a.split(':')[1] != b.split(':')[1]}
                self.assertEqual(relevant, set())

    def test_both_opt_ins_retain_mode_rules_without_reviewed_bus_stops(self):
        self.sync()
        plain = self.project()
        self.assertEqual(len(plain['transitions']), 6)
        self.assertEqual(plain['bus_stop_positions'], [])
        self.assertNotIn(native.PLAYER_BUS_STOPS, plain)
        self.policy[native.PLAYER_BUS_STOPS] = True
        self.assertEqual(self.project(), {**plain, native.PLAYER_BUS_STOPS: True})

    def test_cli_keeps_simulation_label_with_candidate_receipt(self):
        self.sync()
        config = self.root / 'policy.json'
        result = self.root / 'asset.json'
        self.policy[native.PLAYER_BUS_STOPS] = True
        config.write_bytes(build.encoded(self.policy))
        process = subprocess.run([sys.executable, str(Path(native.__file__)), str(self.source),
                                  '--coverage', str(COVERAGE), '--policy', str(config),
                                  '--junction-coverage', str(self.certificate), '--output', str(result)],
                                 capture_output=True, check=True)
        receipt = native.json.loads(process.stdout)
        self.assertEqual(receipt['movement_policy'], 'simulated-simple-at-grade-v1')
        self.assertEqual(receipt['simulated_junctions_sha256'], self.policy[native.JUNCTION_CERTIFICATE])
        self.assertEqual(receipt['sha256'], build.sha(result.read_bytes()))
        self.assertTrue(receipt['candidate_only'])
        self.assertIs(receipt[native.PLAYER_BUS_STOPS], True)


if __name__ == '__main__':
    unittest.main()
