import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location('water_mask_build', Path(__file__).with_name('build.py'))
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)

class MaskBuildTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
    def tearDown(self):
        self.temp.cleanup()
    def source(self, name='source', bounds=None, observations=None, issues=None):
        root = self.root/name
        root.mkdir()
        bounds = bounds or [139.,35.,139.001,35.001]
        payload = dict(format='rosenfu-water-evidence',version=1,bounds=bounds,revision='source-v1',observations=observations or [],source_issues=issues or [])
        raw = builder.encoded(payload)
        (root/'evidence.json').write_bytes(raw)
        manifest = dict(format='rosenfu-water-evidence-manifest',version=1,bounds=bounds,revision='source-v1',payload=dict(file='evidence.json',bytes=len(raw),sha256=builder.sha(raw)),source=dict(url='https://example.org/source.pbf',file_sha256='a'*64,license='ODbL-1.0',attribution='© OpenStreetMap contributors'))
        mraw = builder.encoded(manifest)
        (root/'manifest.json').write_bytes(mraw)
        c = dict(format='rosenfu-water-composite',version=1,parts=[dict(key='test',manifest='manifest.json',manifest_sha256=builder.sha(mraw),payload='evidence.json')])
        craw=builder.encoded(c)
        (root/'composite.json').write_bytes(craw)
        return root,builder.sha(craw)
    def area(self,tags=None):
        return dict(id='way/1',kind='water-area',tags=tags or {},geometry=dict(type='Polygon',coordinates=[[[139.,35.],[139.001,35.],[139.001,35.001],[139.,35.001],[139.,35.]]]))
    def test_empty_input_is_unknown_and_keeps_source_issues(self):
        src,pin=self.source(issues=[{'id':'relation/1','reason':'unassembled'}])
        result=builder.build(src,pin,self.root/'output')
        self.assertEqual(result['water_cells'],0)
        self.assertEqual(result['unknown_cells'],result['spec']['width']*result['spec']['height'])
        self.assertEqual(result['spec']['coverage'],'observations_only')
        self.assertEqual(len(result['source_issues']),1)
        self.assertEqual(result['coverage'],[])
    def test_observed_surface_and_excluded_covered_water_are_separate(self):
        src,pin=self.source(observations=[self.area()])
        result=builder.build(src,pin,self.root/'wet')
        self.assertGreater(result['water_cells'],0)
        covered,cpin=self.source('covered',observations=[self.area({'covered':'yes'})])
        excluded=builder.build(covered,cpin,self.root/'excluded')
        self.assertEqual(excluded['water_cells'],0)
        self.assertEqual(excluded['excluded_area_observations'],1)
    def test_revision_is_source_based_not_cell_based(self):
        one,pin1=self.source('one')
        two,pin2=self.source('two',bounds=[139.001,35.,139.002,35.001])
        a=builder.build(one,pin1,self.root/'a');b=builder.build(two,pin2,self.root/'b')
        self.assertNotEqual(a['spec']['id'],b['spec']['id'])
        self.assertEqual(a['spec']['revision'],b['spec']['revision'])
        old = builder.sha(builder.encoded(dict(policy='observed-surface-centres-v1', sources=sorted(a['sources'], key=lambda x:x['key']))))
        self.assertNotEqual(a['spec']['revision'], old)
    def test_pin_and_geometry_fail_before_output(self):
        src,pin=self.source()
        with self.assertRaises(ValueError):builder.build(src,'0'*64,self.root/'bad-pin')
        self.assertFalse((self.root/'bad-pin').exists())
        a=self.area();a['geometry']['coordinates']=[[[139.,35.],[139.001,35.001],[139.,35.001],[139.001,35.],[139.,35.]]]
        invalid,ipin=self.source('invalid',observations=[a])
        with self.assertRaises(ValueError):builder.build(invalid,ipin,self.root/'bad-shape')
        self.assertFalse((self.root/'bad-shape').exists())
    def test_no_existing_output_overwrite_or_unbounded_grid(self):
        src,pin=self.source()
        out=self.root/'protected';out.mkdir();(out/'keep').write_text('retained')
        with self.assertRaises(ValueError):builder.build(src,pin,out)
        self.assertEqual((out/'keep').read_text(),'retained')
        huge,hpin=self.source('huge',bounds=[130.,30.,140.,40.])
        with self.assertRaises(ValueError):builder.build(huge,hpin,self.root/'huge-output')
        self.assertFalse((self.root/'huge-output').exists())
    def test_input_traversal_is_rejected(self):
        src,pin=self.source()
        with self.assertRaises(ValueError):builder.read(src,'../anything','0'*64)


    def test_rounded_extent_matches_decoder_domain(self):
        for i,bounds in enumerate([[179.999,35.,180.,35.001],[-180.,35.,-179.999,35.001],[139.,84.999,139.001,85.],[139.,-85.,139.001,-84.999]]):
            src,pin=self.source('edge-'+str(i),bounds=bounds)
            out=self.root/('edge-output-'+str(i))
            with self.assertRaises(ValueError):builder.build(src,pin,out)
            self.assertFalse(out.exists())
    def test_contradictory_basin_stays_unknown(self):
        for i,tags in enumerate([{'landuse':'basin','basin':'detention','natural':'water'},{'water':'basin','natural':'water'},{'basin':'retention','natural':'water'}]):
            src,pin=self.source('basin-'+str(i),observations=[self.area(tags)])
            r=builder.build(src,pin,self.root/('basin-output-'+str(i)))
            self.assertEqual(r['water_cells'],0)
            self.assertEqual(r['excluded_area_observations'],1)

if __name__=='__main__': unittest.main()
