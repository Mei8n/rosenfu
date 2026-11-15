import contextlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
import from_pbf
import build


class GridContractTests(unittest.TestCase):
    def test_filter_bounds_include_every_emitted_cell_at_roundoff_boundaries(self):
        boxes=[b for _,b in build.grid_cells([134.04,34.32,134.08,34.34])]
        self.assertLessEqual(min(b[0] for b in boxes),134.04)
        self.assertGreaterEqual(max(b[3] for b in boxes),34.34)
        # This regression deliberately exercises the extra western cell.
        self.assertIn('6701_1716',dict(build.grid_cells([134.04,34.32,134.08,34.34])))

    def test_hole_after_another_outer_ring_cannot_certify_hole_as_coverage(self):
        with tempfile.TemporaryDirectory() as temp:
            path=Path(temp)/'source.poly'
            path.write_text('fixture\n1\n0 0\n1 0\n1 1\n0 1\n0 0\nEND\n2\n2 2\n3 2\n3 3\n2 3\n2 2\nEND\n!hole\n.4 .4\n.6 .4\n.6 .6\n.4 .6\n.4 .4\nEND\nEND\n')
            with self.assertRaisesRegex(ValueError,'holes'):
                from_pbf.poly_geojson(path,[(.45,.45,.55,.55)])


@unittest.skipUnless(importlib.util.find_spec('osmium'),'optional pinned pyosmium is not installed')
class PbfTests(unittest.TestCase):
    def setUp(self):
        import osmium
        self.osmium=osmium
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.base=Path(self.temp.name);self.pbf=self.base/'source.osm.pbf';self.poly=self.base/'source.poly';self.out=self.base/'out'
        self.poly.write_text('fixture\n1\n134 34\n134.2 34\n134.2 34.5\n134 34.5\n134 34\nEND\nEND\n')

    def write(self, nodes, ways):
        h=self.osmium.io.Header();h.set('osmosis_replication_timestamp','2026-11-14T21:20:41Z')
        with self.osmium.SimpleWriter(str(self.pbf),header=h) as writer:
            for ident,lon,lat in nodes:
                writer.add_node(self.osmium.osm.mutable.Node(id=ident,version=1,location=(lon,lat)))
            for ident,refs,tags in ways:
                writer.add_way(self.osmium.osm.mutable.Way(id=ident,version=2,nodes=refs,tags=tags))

    def run_pack(self):
        with contextlib.redirect_stdout(io.StringIO()):
            from_pbf.generate(self.pbf,self.poly,[134.04,34.32,134.08,34.34],
                'https://download.geofabrik.de/fixture.osm.pbf',self.out)
        return json.loads((self.out/'manifest.json').read_text())

    def test_roundoff_extra_cell_keeps_road_and_full_crossing_way(self):
        big=90071992547409933
        self.write([(1,134.03,34.33),(2,134.035,34.33),(3,134.,34.33),(4,134.1,34.33)],
            [(big,[1,2],{'highway':'service'}),(20,[3,4],{'highway':'residential'}),(21,[1,2],{'highway':'service','area':'yes'})])
        m=self.run_pack();cell=json.loads((self.out/m['cells']['6701_1716']['file']).read_text())
        self.assertIn(str(big),[w['id'] for w in cell['ways']]);self.assertNotIn('21',[w['id'] for w in cell['ways']])
        crossing=next(w for w in cell['ways'] if w['id']=='20')
        self.assertEqual(crossing['coordinates'],[[134.,34.33],[134.1,34.33]])
        self.assertEqual(crossing['node_ids'],['3','4'])
        self.assertEqual(m['source']['parent_timestamp'],m['revision'])
        self.assertEqual(m['source']['pyosmium'],'4.3.2')

    def test_missing_source_node_never_publishes(self):
        self.write([(1,134.03,34.33)],[(10,[1,2],{'highway':'service'})])
        with self.assertRaises((ValueError,RuntimeError)):
            self.run_pack()
        self.assertFalse(self.out.exists())
