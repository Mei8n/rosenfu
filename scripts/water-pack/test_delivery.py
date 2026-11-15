import json
import tempfile
import unittest
from pathlib import Path
from compose import compose
from delivery import build_delivery
import test_compose as fixtures

class DeliveryTests(unittest.TestCase):
    def fixture(self,root):
        a,b=fixtures.CompositeTests().fixture(root)
        c=compose(root/'inland',a,root/'marine',b,root/'composite')
        return [dict(key='trial',directory=str(root/'composite'),composite_sha256=c['composite_sha256'])]

    def test_static_release_keeps_pins_dates_and_unknown(self):
        with tempfile.TemporaryDirectory() as root:
            root=Path(root);entries=self.fixture(root);r=build_delivery(entries,root/'release')
            self.assertEqual(r['cells'],1);m=json.loads((root/'release/delivery.json').read_text());self.assertEqual(m['coverage'],[]);self.assertNotIn('license',m);self.assertNotIn('attribution',m);self.assertEqual(m['license_policy'],'retain-per-part-source-metadata')
            self.assertEqual(m['cells'][0]['revisions'],dict(inland='inland',marine='marine'))
            pin=entries[0]['composite_sha256'];self.assertEqual((root/'release'/pin/'composite.json').read_bytes(),(root/'composite/composite.json').read_bytes())

    def test_bad_or_missing_source_never_gets_completion_marker(self):
        for mutation in ('pin','payload','duplicate'):
            with self.subTest(mutation=mutation),tempfile.TemporaryDirectory() as root:
                root=Path(root);entries=self.fixture(root)
                if mutation=='pin':entries[0]['composite_sha256']='0'*64
                elif mutation=='payload':(root/'composite/marine/evidence.json').unlink()
                else:entries.append(entries[0])
                with self.assertRaises((ValueError,OSError)):build_delivery(entries,root/'release')
                self.assertFalse((root/'release/delivery.json').exists())
