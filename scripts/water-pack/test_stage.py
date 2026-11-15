import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from build import encoded
from source_set import audit_sources
from stage import stage_region,extract_stage,_scan_region
import test_source_set as fixtures

@unittest.skipUnless(fixtures.AVAILABLE,'optional osmium not installed')
class StageTests(unittest.TestCase):
    def fixture(self,root):
        receipts=fixtures.SourceSetTests().fixture(root);source_set=encoded(audit_sources(receipts,'2026-11-14T21:20:41Z'))
        path=root/'set.json';path.write_bytes(source_set)
        return path,hashlib.sha256(source_set).hexdigest()

    def test_full_stage_and_bounded_unknown_query(self):
        with tempfile.TemporaryDirectory() as root:
            root=Path(root);manifest,pin=self.fixture(root)
            def read(path,collector,**options):
                collector.source_header={'osmosis_replication_timestamp':'2026-11-14T21:20:41Z'}
                for identity,x in [('one',134),('two',135)]:collector.add(identity,'waterway-line',{'type':'LineString','coordinates':[[x,34],[x+.01,34.01]]},{'waterway':'stream'})
                collector.issues=[{'id':'relation/3','reason':'invalid-area'}]
            with patch('stage._scan_isolated',side_effect=_scan_region),patch('stage.read_osm',read):report=stage_region(root/'kanto-261114.osm.pbf',manifest,pin,'kanto',root/'stage')
            self.assertEqual(report['observations'],2)
            result=extract_stage(root/'stage',report['stage_manifest_sha256'],[134,34,134.05,34.05],root/'pack')
            self.assertEqual(result['counts']['waterway-line'],1)
            data=json.loads((root/'pack/evidence.json').read_text());self.assertEqual(data['coverage'],[]);self.assertEqual(len(data['source_issues']),1)
            extract_stage(root/'stage',report['stage_manifest_sha256'],[136,34,136.05,34.05],root/'empty')
            self.assertEqual(json.loads((root/'empty/evidence.json').read_text())['observations'],[])
            db=root/'stage/stage.sqlite';db.write_bytes(db.read_bytes()+b'changed')
            with self.assertRaisesRegex(ValueError,'mismatch'):extract_stage(root/'stage',report['stage_manifest_sha256'],[134,34,134.05,34.05],root/'bad')

    def test_real_pbf_stage_and_bad_pin(self):
        with tempfile.TemporaryDirectory() as root:
            root=Path(root);manifest,pin=self.fixture(root)
            with self.assertRaisesRegex(ValueError,'pin'):stage_region(root/'kanto-261114.osm.pbf',manifest,'0'*64,'kanto',root/'bad')
            self.assertFalse((root/'bad').exists())
            report=stage_region(root/'kanto-261114.osm.pbf',manifest,pin,'kanto',root/'stage')
            self.assertEqual(report['observations'],0)
            self.assertFalse((root/'stage/locations.idx').exists())
            extract_stage(root/'stage',report['stage_manifest_sha256'],[134,34,134.05,34.05],root/'pack')
            self.assertEqual(json.loads((root/'pack/manifest.json').read_text())['coverage'],[])

    def test_repinned_bad_provenance_is_not_a_valid_source_set(self):
        for field,value in [('url','http://source.test'),('osm_timestamp','2025-01-01T00:00:00Z')]:
            with self.subTest(field=field),tempfile.TemporaryDirectory() as root:
                root=Path(root);manifest,_=self.fixture(root);data=json.loads(manifest.read_text())
                next(e for e in data['sources'] if e['region']=='kanto')[field]=value
                raw=encoded(data);manifest.write_bytes(raw)
                with self.assertRaises(ValueError):stage_region(root/'kanto-261114.osm.pbf',manifest,hashlib.sha256(raw).hexdigest(),'kanto',root/'stage')
                self.assertFalse((root/'stage').exists())

    def test_changed_input_does_not_publish_completed_manifest(self):
        with tempfile.TemporaryDirectory() as root:
            root=Path(root);manifest,pin=self.fixture(root)
            def read(path,collector,**options):
                collector.source_header={'osmosis_replication_timestamp':'2026-11-14T21:20:41Z'}
                path.write_bytes(b'changed')
            with patch('stage._scan_isolated',side_effect=_scan_region),patch('stage.read_osm',read):
                with self.assertRaisesRegex(ValueError,'changed'):stage_region(root/'kanto-261114.osm.pbf',manifest,pin,'kanto',root/'stage')
            self.assertFalse((root/'stage/stage-manifest.json').exists())

    def test_worker_failure_does_not_publish_manifest(self):
        with tempfile.TemporaryDirectory() as root:
            root=Path(root);manifest,pin=self.fixture(root)
            with patch('stage.subprocess.run') as run:
                run.return_value.returncode=9;run.return_value.stderr='worker failed'
                with self.assertRaisesRegex(ValueError,'exit 9'):
                    stage_region(root/'kanto-261114.osm.pbf',manifest,pin,'kanto',root/'stage')
            self.assertFalse((root/'stage/stage-manifest.json').exists())
