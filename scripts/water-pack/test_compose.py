import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from build import build,encoded
from compose import compose

class CompositeTests(unittest.TestCase):
    def fixture(self,root):
        source=root/'source.json';source.write_bytes(encoded(dict(type='FeatureCollection',features=[])))
        pins=[]
        for name in ('inland','marine'):
            result=build(source,[134,34,134.1,34.1],name,'https://source.test','ODbL-1.0','https://license.test','OSM',root/name)
            pins.append(result['manifest_sha256'])
        return pins

    def test_copies_independent_bytes_and_revisions(self):
        with tempfile.TemporaryDirectory() as root:
            root=Path(root);a,b=self.fixture(root)
            report=compose(root/'inland',a,root/'marine',b,root/'composite')
            self.assertEqual(report['revisions'],dict(inland='inland',marine='marine'))
            index=json.loads((root/'composite/composite.json').read_text());self.assertEqual(index['coverage'],[])
            for key in ('inland','marine'):
                for filename in ('manifest.json','evidence.json'):self.assertEqual((root/key/filename).read_bytes(),(root/'composite'/key/filename).read_bytes())
            with self.assertRaises(ValueError):compose(root/'inland',a,root/'marine',b,root/'composite')

    def test_missing_part_and_repinned_complete_part_fail(self):
        with tempfile.TemporaryDirectory() as root:
            root=Path(root);a,b=self.fixture(root)
            with self.assertRaises(ValueError):compose(root/'missing',a,root/'marine',b,root/'bad')
            path=root/'marine/manifest.json';m=json.loads(path.read_text());m['geographic_completeness']='complete';raw=encoded(m);path.write_bytes(raw)
            with self.assertRaises(ValueError):compose(root/'inland',a,root/'marine',hashlib.sha256(raw).hexdigest(),root/'bad')
            self.assertFalse((root/'bad').exists())

    def test_repinned_incomplete_leaf_schema_fails_before_publication(self):
        for field,value in [('revision',None),('source',None),('limitations',[])]:
            with self.subTest(field=field),tempfile.TemporaryDirectory() as root:
                root=Path(root);a,b=self.fixture(root);path=root/'marine/manifest.json'
                m=json.loads(path.read_text());m[field]=value;raw=encoded(m);path.write_bytes(raw)
                with self.assertRaises(ValueError):compose(root/'inland',a,root/'marine',hashlib.sha256(raw).hexdigest(),root/'bad')
                self.assertFalse((root/'bad').exists())

    def test_multiple_regions_preserve_parts_without_union(self):
        from compose import compose_many
        with tempfile.TemporaryDirectory() as root:
            root=Path(root);a,b=self.fixture(root)
            r=compose_many([('inland_a',root/'inland',a),('inland_b',root/'inland',a),('marine',root/'marine',b)],root/'many')
            self.assertEqual(list(r['revisions']),['inland_a','inland_b','marine'])
            from delivery import build_delivery
            d=build_delivery([dict(key='border',directory=str(root/'many'),composite_sha256=r['composite_sha256'])],root/'release')
            self.assertEqual(d['cells'],1)
            self.assertEqual((root/'many/composite.json').read_bytes(),(root/'release'/r['composite_sha256']/'composite.json').read_bytes())

    def test_extra_counts_cannot_bypass_composite_observation_limit(self):
        from compose import compose_many
        with tempfile.TemporaryDirectory() as root:
            root=Path(root);self.fixture(root);directory=root/'inland'
            payload=json.loads((directory/'evidence.json').read_text())
            payload['observations']=[dict(id=str(i),kind='waterway-line',geometry=dict(type='LineString',coordinates=[[134.01,34.01],[134.02,34.02]]),tags={}) for i in range(7000)]
            raw=encoded(payload);(directory/'evidence.json').write_bytes(raw)
            m=json.loads((directory/'manifest.json').read_text());m['payload'].update(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),counts={'water-area':0,'waterway-line':7000,'coastline':0,'unknown':-10000})
            raw=encoded(m);(directory/'manifest.json').write_bytes(raw);pin=hashlib.sha256(raw).hexdigest()
            with self.assertRaisesRegex(ValueError,'geometry limit'):compose_many([(key,directory,pin) for key in ('a','b','c')],root/'bad')
            self.assertFalse((root/'bad').exists())
