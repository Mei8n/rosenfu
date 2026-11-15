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
