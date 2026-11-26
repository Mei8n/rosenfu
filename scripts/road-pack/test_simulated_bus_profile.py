from pathlib import Path
import tempfile
import unittest
import xml.etree.ElementTree as ET

import build
import build_native_region as native


class SimulatedBusProfileTests(unittest.TestCase):
    def setUp(self):
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        temp = Path(scratch.name)
        self.raw, self.coverage, self.cert = (temp / name for name in ('test.osm', 'coverage.json', 'certificate.json'))
        self.coverage.write_bytes(build.encoded({'type': 'Polygon', 'coordinates': [[
            [138.98, 34.98], [139.04, 34.98], [139.04, 35.04], [138.98, 35.04], [138.98, 34.98],
        ]]}))
        self.policy = {
            'format': 1, 'asset_id': 'synthetic-profile', 'source_sha256': '',
            'coverage_sha256': build.sha(self.coverage.read_bytes()), 'revision': '2026-11-14T21:20:41Z',
            'origin': 'Fictional fixture', 'region': 'Test region', 'bounds': [139.0, 35.0, 139.02, 35.02],
            'modes': ['bus', 'tram'], 'turns': [], 'bus_stop_positions': [], 'allows_player_bus_stops': True,
        }

    def setup_source(self, tags=None, branch_tags=None, *, metadata=True, barrier=False, restriction=None,
                     restriction_kind='no_right_turn', fourth=None, intersect_only=False, split=False, attribution=True):
        root = ET.Element('osm', version='0.6')
        if metadata:
            ET.SubElement(root, 'meta', osm_base=self.policy['revision'])
        coordinates = [
            (139.002, 35.01), (139.006, 35.01), (139.010, 35.01),
            (139.006, 35.013), (139.002, 35.017), (139.010, 35.017),
        ]
        if fourth is not None:
            coordinates.append(fourth)
            if intersect_only:
                coordinates.append(coordinates[1])  # identical geometry but distinct raw node
        for n, (lon, lat) in enumerate(coordinates, 1):
            node = ET.SubElement(root, 'node', id=str(n), lon=str(lon), lat=str(lat))
            if barrier and n == 2:
                ET.SubElement(node, 'tag', k='barrier', v='gate')
        attribution_tags = {'source': 'test survey'} if attribution else {}
        branch = {'highway': 'unclassified', **attribution_tags, **(branch_tags or {})}
        cross_node = 8 if intersect_only else 2
        ways = [
            ('100', [1, 2, 3], {'highway': 'residential', **attribution_tags, **(tags or {})}),
            ('200', [4, cross_node, 7] if fourth is not None and not split else [cross_node, 4], branch),
            ('300', [5, 6], {'highway': 'residential', 'oneway': 'no', 'bus': 'yes', 'tram': 'yes'}),
        ]
        if split:
            ways.append(('201', [cross_node, 7], branch))
        for n, refs, values in ways:
            way = ET.SubElement(root, 'way', id=n, version='1')
            for ref in refs:
                ET.SubElement(way, 'nd', ref=str(ref))
            for k, v in sorted(values.items()):
                ET.SubElement(way, 'tag', k=k, v=v)
        if restriction:
            relation = ET.SubElement(root, 'relation', id='500')
            ET.SubElement(relation, 'tag', k='type', v='restriction:bus')
            ET.SubElement(relation, 'tag', k='restriction:bus', v=restriction_kind)
            for kind, ref, role in [('way', '100', 'from'), ('node', '2', 'via'), ('way', restriction, 'to')]:
                ET.SubElement(relation, 'member', type=kind, ref=ref, role=role)
        self.raw.write_bytes(ET.tostring(root))
        self.policy['source_sha256'] = build.sha(self.raw.read_bytes())

    def certify(self, **changes):
        cert = dict({key: self.policy[key] for key in ('source_sha256', 'coverage_sha256', 'revision', 'bounds')},
                    format=1, profile=native.BUS_PROFILE, country='JP', raw_way_access_and_direction_tags_complete=True,
                    raw_node_tags_complete=True, incident_ways_complete=True, restriction_relations_complete=True)
        cert.update(changes)
        self.cert.write_bytes(build.encoded(cert))
        return dict(self.policy, simulated_bus_profile_sha256=build.sha(self.cert.read_bytes()))

    def profile(self):
        return native.project(self.raw, self.coverage, self.certify(), bus_profile_coverage=self.cert)

    def test_default_remains_strict_and_omitted_direction_is_only_simulated_for_bus(self):
        self.setup_source({'tram': 'yes'})
        default = native.project(self.raw, self.coverage, self.policy)
        self.assertEqual([e['id'] for e in default['edges']], ['osm:300:0:forward', 'osm:300:0:reverse'])
        self.assertNotIn('simulated bus', default['origin'])
        profile = self.profile()
        ways = {e['id']: e for e in profile['edges']}
        self.assertTrue(profile['allows_player_bus_stops'])
        self.assertEqual(profile['bus_stop_positions'], [])
        self.assertIn('simulated bus access', profile['origin'])
        for direction in ('forward', 'reverse'):
            for segment in (0, 1):
                edge = ways[f'osm:100:{segment}:{direction}']
                self.assertEqual((edge['bus_access'], edge['tram_access']), ('allowed', 'unknown'))
        self.assertEqual(ways['osm:300:0:forward']['tram_access'], 'allowed')

    def test_both_explicit_directions_and_at_grade_same_way_through_only(self):
        for direction, allowed in [('no', {'forward', 'reverse'}), ('yes', {'forward'}), ('-1', {'reverse'})]:
            with self.subTest(direction=direction):
                self.setup_source({'oneway': direction})
                profile = self.profile()
                self.assertEqual({e['id'].split(':')[-1] for e in profile['edges'] if e['id'].startswith('osm:100:')}, allowed)
                expected = set()
                if 'forward' in allowed:
                    expected.add(('bus', 'osm:100:0:forward', 'osm:100:1:forward'))
                if 'reverse' in allowed:
                    expected.add(('bus', 'osm:100:1:reverse', 'osm:100:0:reverse'))
                self.assertEqual({(t['mode'], t['incoming'], t['outgoing']) for t in profile['transitions']}, expected)
                self.assertNotIn('simulated turns', profile['origin'])
        self.setup_source(branch_tags={'access': 'private'})
        self.assertEqual(self.profile()['transitions'], [])  # unknown/denied arm blocks shared through

    @staticmethod
    def movements(asset):
        return {(t['mode'], t['incoming'], t['outgoing']) for t in asset['transitions']}

    def test_certified_four_arm_continues_on_each_original_way_only(self):
        self.setup_source(fourth=(139.006, 35.007), tags={'oneway': '-1'})
        asset = self.profile()
        self.assertEqual(self.movements(asset), {
            ('bus', 'osm:100:1:reverse', 'osm:100:0:reverse'),
            ('bus', 'osm:200:0:forward', 'osm:200:1:forward'),
            ('bus', 'osm:200:1:reverse', 'osm:200:0:reverse'),
        })
        self.assertTrue(all(e['tram_access'] == 'unknown' for e in asset['edges'] if e['id'].startswith(('osm:100:', 'osm:200:'))))
        strict = {'bus': 'yes', 'tram': 'yes', 'oneway': 'no'}
        self.setup_source(fourth=(139.006, 35.007), tags=strict, branch_tags=strict, attribution=False)
        # Even explicitly permitted ground edges don't gain a new shared-node
        # movement when the gameplay source profile is off.
        baseline = native.project(self.raw, self.coverage, self.policy)
        self.assertEqual(len(baseline['edges']), 10)
        self.assertEqual(self.movements(baseline), set())
        junction_certificate = self.cert.with_name('junctions.json')
        junction_certificate.write_bytes(build.encoded({
            **{k: self.policy[k] for k in ('source_sha256', 'coverage_sha256', 'revision', 'bounds')},
            'format': 1, 'incident_ways_complete': True, 'restriction_relations_complete': True,
        }))
        both = self.certify()
        both['simulated_junctions_sha256'] = build.sha(junction_certificate.read_bytes())
        with_both = native.project(self.raw, self.coverage, both, bus_profile_coverage=self.cert,
                                   junction_coverage=junction_certificate)
        self.assertEqual(self.movements(with_both), {
            ('bus', 'osm:100:0:forward', 'osm:100:1:forward'),
            ('bus', 'osm:100:1:reverse', 'osm:100:0:reverse'),
            ('bus', 'osm:200:0:forward', 'osm:200:1:forward'),
            ('bus', 'osm:200:1:reverse', 'osm:200:0:reverse'),
        })

    def test_four_arm_prohibitions_and_ambiguous_approaches_remain_blocked(self):
        fourth = (139.006, 35.007)
        for flags in [
            {'restriction': '100', 'restriction_kind': 'no_straight_on'},
            {'barrier': True},
            {'branch_tags': {'bus': 'no'}},
            {'branch_tags': {'layer': '1'}},
            {'branch_tags': {'oneway:conditional': 'yes @ (08:00-09:00)'}},
            {'split': True},
        ]:
            with self.subTest(flags=flags):
                self.setup_source(fourth=fourth, **flags)
                self.assertEqual(self.movements(self.profile()), set())
        # Repeated coordinates, same-side arms, and a badly skewed corner.
        for bad_fourth in [(139.006, 35.013), (139.006, 35.014), (139.010, 35.009)]:
            with self.subTest(bad_fourth=bad_fourth):
                self.setup_source(fourth=bad_fourth)
                self.assertEqual(self.movements(self.profile()), set())

    def test_certified_halo_arms_are_evidence_not_playable_edges(self):
        self.setup_source(fourth=(139.006, 34.995))  # beyond playable south edge, within proven halo
        profile = self.profile()
        self.assertFalse(any(e['id'].startswith('osm:200:') for e in profile['edges']))
        self.assertEqual(self.movements(profile), {
            ('bus', 'osm:100:0:forward', 'osm:100:1:forward'),
            ('bus', 'osm:100:1:reverse', 'osm:100:0:reverse'),
        })
        for latitude in (34.98, 34.97):  # directly on the certified boundary and beyond
            with self.subTest(latitude=latitude):
                self.setup_source(fourth=(139.006, latitude))
                self.assertEqual(self.movements(self.profile()), set())
        # Both ends inside a concave halo is insufficient: the intervening
        # segment must not go through an uncovered notch outside the inner area.
        notch = [
            [138.98, 34.98], [139.04, 34.98], [139.04, 34.997],
            [139.004, 34.997], [139.004, 34.999], [139.04, 34.999],
            [139.04, 35.04], [138.98, 35.04], [138.98, 34.98],
        ]
        self.coverage.write_bytes(build.encoded({'type': 'Polygon', 'coordinates': [notch]}))
        self.policy['coverage_sha256'] = build.sha(self.coverage.read_bytes())
        self.setup_source(fourth=(139.006, 34.995))
        self.assertEqual(self.movements(self.profile()), set())

    def test_equal_coordinates_do_not_join_crossing_raw_ways(self):
        self.setup_source(fourth=(139.006, 35.007), intersect_only=True)
        asset = self.profile()
        raw = {n['id']: n['point'] for n in asset['nodes']}
        self.assertEqual(raw['osm:2'], raw['osm:8'])
        self.assertEqual(self.movements(asset), {
            ('bus', 'osm:100:0:forward', 'osm:100:1:forward'),
            ('bus', 'osm:100:1:reverse', 'osm:100:0:reverse'),
            ('bus', 'osm:200:0:forward', 'osm:200:1:forward'),
            ('bus', 'osm:200:1:reverse', 'osm:200:0:reverse'),
        })

    def test_explicit_constraints_are_never_replaced_by_the_simulated_default(self):
        for key, value in [
            ('bus', 'no'), ('bus', 'permissive'), ('access', 'private'), ('motor_vehicle', 'no'),
            ('psv', 'destination'), ('bus:conditional', 'no @ (08:00-09:00)'),
            ('access:conditional', 'no @ (Su)'), ('oneway:bus', 'no'), ('oneway', 'reversible'),
            ('junction', 'roundabout'), ('bridge', 'yes'), ('tunnel', 'yes'), ('level', '1'),
            ('surface', 'gravel'), ('maxheight', '3.0'), ('source:date', '2026-08-01'), ('source', ''),
        ]:
            with self.subTest(key=key, value=value):
                self.setup_source({key: value})
                self.assertFalse(any(e['id'].startswith('osm:100:') for e in self.profile()['edges']))

    def test_raw_node_and_restriction_reject_without_coordinate_based_reconnection(self):
        for extra in ({'barrier': True}, {'restriction': '200'}):
            self.setup_source(**extra)
            self.assertFalse(any(e['id'].startswith('osm:100:') for e in self.profile()['edges']))
        self.setup_source(restriction='999')
        with self.assertRaisesRegex(ValueError, 'unresolved restriction member'):
            self.profile()

    def test_certificate_is_independent_and_bound_to_exact_snapshot(self):
        self.setup_source()
        with self.assertRaisesRegex(ValueError, 'both policy opt-in'):
            native.project(self.raw, self.coverage, self.certify())
        for change, error in [
            ({'country': 'FR'}, 'Japanese'),
            ({'raw_node_tags_complete': 1}, 'Japanese'),
            ({'incident_ways_complete': False}, 'Japanese'),
            ({'restriction_relations_complete': False}, 'Japanese'),
            ({'revision': '2026-11-15T21:20:41Z'}, 'exact region snapshot'),
        ]:
            with self.subTest(change=change):
                with self.assertRaisesRegex(ValueError, error):
                    native.project(self.raw, self.coverage, self.certify(**change), bus_profile_coverage=self.cert)
        policy = self.certify()
        self.cert.write_bytes(self.cert.read_bytes()+b' ')
        with self.assertRaisesRegex(ValueError, 'digest mismatch'):
            native.project(self.raw, self.coverage, policy, bus_profile_coverage=self.cert)
        self.setup_source(metadata=False)
        with self.assertRaisesRegex(ValueError, 'raw snapshot revision'):
            self.profile()


if __name__ == '__main__':
    unittest.main()
