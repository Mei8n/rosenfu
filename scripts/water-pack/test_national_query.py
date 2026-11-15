import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from build import encoded
from national_query import build_queries
from source_set import REGIONS

class QueryTests(unittest.TestCase):
    def fixture(self,root):
        stages={}
        for region in sorted(REGIONS):
            directory=root/region;directory.mkdir();raw=encoded(dict(region=region,source_set_sha256='a'*64,geographic_completeness='unknown',coverage=[],source=dict(osm_timestamp='2026-11-14T21:20:41Z')))
            (directory/'stage-manifest.json').write_bytes(raw);stages[region]=dict(directory=str(directory),sha256=hashlib.sha256(raw).hexdigest())
        return dict(stages=stages,source_set_sha256='a'*64,marine=dict(path=str(root/'marine.shp'),bundle_sha256='b'*64),cells=[dict(key='border',bounds=[134,34,134.1,34.1])])

    def test_missing_region_or_mismatched_stage_fails_before_output(self):
        for case in ('missing','source','time'):
            with self.subTest(case=case),tempfile.TemporaryDirectory() as root:
                root=Path(root);c=self.fixture(root)
                if case=='missing':del c['stages']['kanto']
                else:
                    e=c['stages']['kanto'];p=Path(e['directory'])/'stage-manifest.json';m=json.loads(p.read_text())
                    if case=='source':m['source_set_sha256']='c'*64
                    else:m['source']['osm_timestamp']='2026-11-13T21:20:41Z'
                    raw=encoded(m);p.write_bytes(raw);e['sha256']=hashlib.sha256(raw).hexdigest()
                with self.assertRaises(ValueError):build_queries(c,root/'output')
                self.assertFalse((root/'output').exists())

    def test_marine_pin_mismatch_fails_before_output(self):
        with tempfile.TemporaryDirectory() as root:
            root=Path(root);c=self.fixture(root)
            with patch('national_query.source_files',return_value=[]):
                with self.assertRaisesRegex(ValueError,'Marine'):build_queries(c,root/'output')
            self.assertFalse((root/'output').exists())

    def test_real_eight_stage_query_keeps_all_nine_sources(self):
        import test_source_set as source_fixture
        import test_marine as marine_fixture
        if not source_fixture.AVAILABLE or not marine_fixture.AVAILABLE:self.skipTest('optional source dependencies unavailable')
        from source_set import audit_sources
        from stage import stage_region
        from marine import source_files
        with tempfile.TemporaryDirectory() as root:
            root=Path(root);receipts=source_fixture.SourceSetTests().fixture(root)
            raw=encoded(audit_sources(receipts,'2026-11-14T21:20:41Z'));source=root/'sources.json';source.write_bytes(raw);pin=hashlib.sha256(raw).hexdigest();stages={}
            for region in sorted(REGIONS):
                directory=root/region;r=stage_region(root/f'{region}-261114.osm.pbf',source,pin,region,directory)
                stages[region]=dict(directory=str(directory),sha256=r['stage_manifest_sha256'])
            shp=root/'water.shp';marine_fixture.MarineTests().fixture(shp)
            c=dict(stages=stages,source_set_sha256=pin,marine=dict(path=str(shp),bundle_sha256=hashlib.sha256(encoded(source_files(shp))).hexdigest(),revision='2026-11-13T15:00:00Z',url='https://source.test/water'),cells=[dict(key='border',bounds=[134.02,34.02,134.1,34.1])])
            result=build_queries(c,root/'query');self.assertEqual(result['cells'],1)
            release=json.loads((root/'query/release/delivery.json').read_text());cell=release['cells'][0]
            composite=json.loads((root/'query/release'/cell['directory']/'composite.json').read_text())
            self.assertEqual(len(composite['parts']),9);self.assertEqual(composite['coverage'],[])
            self.assertEqual(set(cell['revisions']),{'inland_'+r for r in REGIONS}|{'marine'})
