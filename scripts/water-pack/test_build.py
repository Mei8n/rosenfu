import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

spec=importlib.util.spec_from_file_location('water_builder',Path(__file__).with_name('build.py'))
builder=importlib.util.module_from_spec(spec);spec.loader.exec_module(builder)

class WaterPackTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.root=Path(self.tmp.name)
    def tearDown(self):
        self.tmp.cleanup()
    def build(self,features):
        source=self.root/'source.geojson';source.write_text(json.dumps({'type':'FeatureCollection','features':features}))
        result=builder.build(source,[134,34,134.1,34.1],'test-source-date','https://source.test/water.geojson','test-license','https://source.test/license','test attribution',self.root/'pack')
        return result,json.loads((self.root/'pack/manifest.json').read_text()),json.loads((self.root/'pack/evidence.json').read_text())
    def feature(self,kind,geometry,identity='source/1'):
        return dict(type='Feature',id=identity,properties=dict(evidence_kind=kind,source_tags={'name':'test'}),geometry=geometry)
    def polygon(self):
        return {'type':'Polygon','coordinates':[[[134.01,34.01],[134.09,34.01],[134.09,34.09],[134.01,34.09],[134.01,34.01]],[[134.03,34.03],[134.03,34.04],[134.04,34.04],[134.04,34.03],[134.03,34.03]]]}
    def test_preserves_holes_and_hashes(self):
        geometry=self.polygon();result,m,d=self.build([self.feature('water-area',geometry)])
        self.assertEqual(d['observations'][0]['geometry'],geometry)
        self.assertEqual(m['source']['file_sha256'],hashlib.sha256((self.root/'source.geojson').read_bytes()).hexdigest())
        self.assertEqual(result['manifest_sha256'],hashlib.sha256((self.root/'pack/manifest.json').read_bytes()).hexdigest())
        self.assertEqual(m['payload']['sha256'],hashlib.sha256((self.root/'pack/evidence.json').read_bytes()).hexdigest())
        self.assertEqual(m['coverage'],[]);self.assertEqual(d['coverage'],[])
    def test_empty_is_unknown_never_dry(self):
        _,m,d=self.build([]);self.assertEqual(d['observations'],[]);self.assertEqual(m['geographic_completeness'],'unknown');self.assertEqual(d['geographic_completeness'],'unknown')
    def test_river_and_coast_do_not_become_surfaces(self):
        line={'type':'LineString','coordinates':[[134.02,34.02],[134.08,34.08]]}
        _,m,d=self.build([self.feature('waterway-line',line,'way/1'),self.feature('coastline',line,'way/2')])
        self.assertEqual(m['payload']['counts'],{'water-area':0,'waterway-line':1,'coastline':1})
        self.assertTrue(all(o['geometry']['type']=='LineString' for o in d['observations']))
    def test_full_source_geometry_not_clipped(self):
        line={'type':'LineString','coordinates':[[133.9,34.05],[134.2,34.05]]}
        _,_,d=self.build([self.feature('waterway-line',line)]);self.assertEqual(d['observations'][0]['geometry'],line)
    def test_outside_features_excluded(self):
        _,_,d=self.build([self.feature('coastline',{'type':'LineString','coordinates':[[135,35],[135.1,35.1]]})]);self.assertEqual(d['observations'],[])
    def test_existing_output_is_never_overwritten(self):
        self.build([]);before=(self.root/'pack/manifest.json').read_bytes()
        with self.assertRaisesRegex(ValueError,'already exists'):self.build([])
        self.assertEqual((self.root/'pack/manifest.json').read_bytes(),before)
    def test_invalid_ring_and_nonfinite_coordinates_fail_without_manifest(self):
        for coords in [[[[134,34],[134.1,34],[134,34.1],[134.1,34.1]]],[[[134,34],[float('nan'),34],[134,34.1],[134,34]]]]:
            with self.assertRaises(ValueError):self.build([self.feature('water-area',{'type':'Polygon','coordinates':coords})])
            self.assertFalse((self.root/'pack/manifest.json').exists())
    def test_duplicate_kind_identity_rejected(self):
        f=self.feature('water-area',self.polygon())
        with self.assertRaisesRegex(ValueError,'Duplicate'):self.build([f,f])
    def test_large_area_and_forged_dry_kind_rejected(self):
        with self.assertRaises(ValueError):builder.Collector([130,30,140,40])
        with self.assertRaises(ValueError):self.build([self.feature('verified-dry',self.polygon())])
    def test_expanded_polygon_limit(self):
        geometry={'type':'MultiPolygon','coordinates':[self.polygon()['coordinates'][:1]]*10001}
        with self.assertRaisesRegex(ValueError,'polygon limit'):self.build([self.feature('water-area',geometry)])
    def test_empty_tag_key_rejected(self):
        f=self.feature('water-area',self.polygon());f['properties']['source_tags']={'':'value'}
        with self.assertRaisesRegex(ValueError,'tags'):self.build([f])
    def test_feature_limit(self):
        c=builder.Collector([134,34,134.1,34.1]);c.count=builder.MAX_COORDS
        with self.assertRaises(ValueError):c.add('id','water-area',self.polygon(),{})


@unittest.skipUnless(importlib.util.find_spec('osmium'), 'optional osmium dependency not installed')
class PbfTests(unittest.TestCase):
    def test_unassembled_source_areas_are_not_silently_empty(self):
        import osmium
        with tempfile.TemporaryDirectory() as directory:
            source=Path(directory)/'missing.osm.pbf'
            with osmium.SimpleWriter(str(source)) as writer:
                writer.add_node(osmium.osm.mutable.Node(id=1,location=(134.01,34.01)))
                # Degenerate closed water way and a relation referencing absent ways.
                writer.add_way(osmium.osm.mutable.Way(id=10,nodes=[1,1],tags={'natural':'water'}))
                writer.add_relation(osmium.osm.mutable.Relation(id=20,members=[('w',999,'outer')],tags={'type':'multipolygon','natural':'water'}))
                writer.add_relation(osmium.osm.mutable.Relation(id=21,members=[('w',998,'outer')],tags={'type':'multipolygon','landuse':'forest'}))
                writer.add_relation(osmium.osm.mutable.Relation(id=22,members=[],tags={'type':'waterway','natural':'water'}))
            collector=builder.Collector([135,35,135.1,35.1])
            builder.read_osm(source,collector)
            self.assertEqual(collector.items,[])
            self.assertEqual(collector.issues,[{'id':'relation/20','reason':'water-area-not-assembled'},{'id':'relation/22','reason':'water-area-not-assembled'},{'id':'way/10','reason':'water-area-not-assembled'}])
            _,payload,_=builder.pack_bytes(collector,'fixture',{})
            self.assertEqual(json.loads(payload)['coverage'],[])
            self.assertEqual(json.loads(payload)['geographic_completeness'],'unknown')

    def test_unassembled_issue_limit_fails_without_publishing(self):
        import osmium
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);source=root/'too-many.osm.pbf'
            with osmium.SimpleWriter(str(source)) as writer:
                for identity in range(1,1002):
                    writer.add_relation(osmium.osm.mutable.Relation(id=identity,members=[('w',identity,'outer')],tags={'type':'multipolygon','natural':'water'}))
            with self.assertRaisesRegex(ValueError,'Too many unresolved'):
                builder.build(source,[134,34,134.1,34.1],'fixture','https://source.test/input.pbf','ODbL-1.0','https://opendatacommons.org/licenses/odbl/1-0/','test attribution',root/'pack')
            self.assertFalse((root/'pack').exists())

    def test_relation_hole_and_narrow_river_from_real_pbf_encoding(self):
        import osmium
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);source=root/'synthetic.osm.pbf'
            with osmium.SimpleWriter(str(source)) as writer:
                points=[(134.01,34.01),(134.09,34.01),(134.09,34.09),(134.01,34.09),(134.03,34.03),(134.04,34.03),(134.04,34.04),(134.03,34.04)]
                for i,point in enumerate(points,1):writer.add_node(osmium.osm.mutable.Node(id=i,location=point))
                writer.add_way(osmium.osm.mutable.Way(id=10,nodes=[1,2,3,4,1]))
                writer.add_way(osmium.osm.mutable.Way(id=11,nodes=[5,6,7,8,5]))
                writer.add_way(osmium.osm.mutable.Way(id=12,nodes=[1,3],tags={'waterway':'stream','intermittent':'yes'}))
                writer.add_way(osmium.osm.mutable.Way(id=13,nodes=[2,3],tags={'natural':'coastline'}))
                writer.add_relation(osmium.osm.mutable.Relation(id=20,members=[('w',10,'outer'),('w',11,'inner')],tags={'type':'multipolygon','natural':'water'}))
            result=builder.build(source,[134,34,134.1,34.1],'synthetic','https://source.test/input.pbf','ODbL-1.0','https://opendatacommons.org/licenses/odbl/1-0/','test attribution',root/'pack')
            data=json.loads((root/'pack/evidence.json').read_text())
            self.assertEqual(data['source_issues'],[])
            self.assertEqual(result['counts'],{'water-area':1,'waterway-line':1,'coastline':1})
            area=next(o for o in data['observations'] if o['kind']=='water-area')
            self.assertEqual(area['id'],'relation/20');self.assertEqual(len(area['geometry']['coordinates'][0]),2)
            stream=next(o for o in data['observations'] if o['kind']=='waterway-line')
            self.assertEqual(stream['tags']['intermittent'],'yes');self.assertEqual(data['coverage'],[])

if __name__=='__main__':unittest.main()
