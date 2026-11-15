import hashlib
import tempfile
import unittest
from pathlib import Path
from source_set import audit_sources,REGIONS
try:
    import osmium
    AVAILABLE=True
except ImportError:
    AVAILABLE=False

@unittest.skipUnless(AVAILABLE,'optional osmium not installed')
class SourceSetTests(unittest.TestCase):
    def fixture(self,root,bad_region=None):
        receipts=[]
        for region in sorted(REGIONS):
            path=root/f'{region}-261114.osm.pbf'
            header=osmium.io.Header();header.set('osmosis_replication_timestamp','2026-11-13T21:20:41Z' if region==bad_region else '2026-11-14T21:20:41Z');header.set('generator','fixture')
            with osmium.SimpleWriter(str(path),header=header) as writer:
                writer.add_node(osmium.osm.mutable.Node(id=1,location=(134,34)))
            receipts.append(dict(region=region,path=str(path),source_url='https://download.geofabrik.de/asia/japan/'+path.name,status='verified-transfer',bytes=path.stat().st_size,sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
        return receipts

    def test_shared_snapshot_is_still_unknown(self):
        with tempfile.TemporaryDirectory() as root:
            receipts=self.fixture(Path(root));result=audit_sources(receipts,'2026-11-14T21:20:41Z')
            self.assertEqual(len(result['sources']),8);self.assertEqual(result['coverage'],[])
            self.assertEqual(result['geographic_completeness'],'unknown')
            self.assertNotIn('path',result['sources'][0])

    def test_mismatched_timestamp_and_missing_region_fail(self):
        with tempfile.TemporaryDirectory() as root:
            receipts=self.fixture(Path(root),'kanto')
            with self.assertRaisesRegex(ValueError,'timestamp'): audit_sources(receipts,'2026-11-14T21:20:41Z')
            with self.assertRaises(ValueError):audit_sources(receipts[:-1],'2026-11-14T21:20:41Z')

    def test_changed_bytes_and_duplicate_region_fail(self):
        with tempfile.TemporaryDirectory() as root:
            receipts=self.fixture(Path(root));receipts[0]['sha256']='0'*64
            with self.assertRaisesRegex(ValueError,'hash'):audit_sources(receipts,'2026-11-14T21:20:41Z')
            with self.assertRaises(ValueError):audit_sources([receipts[1]]*8,'2026-11-14T21:20:41Z')

    def test_source_changed_between_hash_and_header_is_rejected(self):
        from unittest.mock import patch
        from build import digest_file
        with tempfile.TemporaryDirectory() as root:
            receipts=self.fixture(Path(root),'kanto')
            replacement=(Path(root)/'shikoku-261114.osm.pbf').read_bytes()
            changed=False
            def swapping_digest(path):
                nonlocal changed
                value=digest_file(path)
                if path.name=='kanto-261114.osm.pbf' and not changed:
                    changed=True;path.write_bytes(replacement)
                return value
            with patch('source_set.digest_file',swapping_digest):
                with self.assertRaisesRegex(ValueError,'changed'):
                    audit_sources(receipts,'2026-11-14T21:20:41Z')
