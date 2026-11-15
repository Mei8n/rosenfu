import json
import tempfile
import unittest
from pathlib import Path
from marine import build_marine
try:
    import shapefile
    from pyproj import CRS
    import shapely
    AVAILABLE=True
except ImportError:
    AVAILABLE=False

@unittest.skipUnless(AVAILABLE,'optional marine dependencies not installed')
class MarineTests(unittest.TestCase):
    def fixture(self,path,epsg=4326,invalid=False):
        with shapefile.Writer(str(path),shapeType=shapefile.POLYGON) as writer:
            writer.field('id','N')
            outer=[[134,34],[134,34.2],[134.2,34.2],[134.2,34],[134,34]]
            hole=[[134.04,34.04],[134.06,34.04],[134.06,34.06],[134.04,34.06],[134.04,34.04]]
            if invalid: outer=[[134,34],[134.2,34.2],[134,34.2],[134.2,34],[134,34]]
            writer.poly([outer,hole] if not invalid else [outer]);writer.record(1)
        path.with_suffix('.prj').write_text(CRS.from_epsg(epsg).to_wkt())

    def test_clip_preserves_island_and_unknown(self):
        with tempfile.TemporaryDirectory() as root:
            p=Path(root); self.fixture(p/'water.shp')
            report=build_marine(p/'water.shp',[134.02,34.02,134.1,34.1],'fixture','https://source.test',p/'pack')
            data=json.loads((p/'pack/evidence.json').read_text()); manifest=json.loads((p/'pack/manifest.json').read_text())
            self.assertEqual(data['coverage'],[]); self.assertEqual(len(data['observations']),1)
            self.assertEqual(len(data['observations'][0]['geometry']['coordinates']),2)
            self.assertEqual(len(manifest['source']['files']),4)
            self.assertEqual(manifest['source']['file_sha256'],report['source_bundle_sha256'])
            for ring in data['observations'][0]['geometry']['coordinates']:
                for x,y in ring: self.assertTrue(134.02<=x<=134.1 and 34.02<=y<=34.1)

    def test_wrong_projection_and_invalid_geometry_fail(self):
        for epsg,invalid in [(3857,False),(4326,True)]:
            with self.subTest(epsg=epsg,invalid=invalid),tempfile.TemporaryDirectory() as root:
                p=Path(root);self.fixture(p/'water.shp',epsg,invalid)
                with self.assertRaises(ValueError): build_marine(p/'water.shp',[134.02,34.02,134.1,34.1],'fixture','https://source.test',p/'pack')
                self.assertFalse((p/'pack').exists())

    def test_no_intersection_remains_unknown(self):
        with tempfile.TemporaryDirectory() as root:
            p=Path(root);self.fixture(p/'water.shp')
            build_marine(p/'water.shp',[135,34,135.1,34.1],'fixture','https://source.test',p/'pack')
            data=json.loads((p/'pack/evidence.json').read_text())
            self.assertEqual(data['observations'],[]);self.assertEqual(data['geographic_completeness'],'unknown')

    def test_orphan_hole_is_not_promoted_to_water(self):
        with tempfile.TemporaryDirectory() as root:
            p=Path(root); path=p/'water.shp'
            with shapefile.Writer(str(path),shapeType=shapefile.POLYGON) as writer:
                writer.field('id','N')
                writer.poly([[[134,34],[134.1,34],[134.1,34.1],[134,34.1],[134,34]]]);writer.record(1)
            path.with_suffix('.prj').write_text(CRS.from_epsg(4326).to_wkt())
            with self.assertRaisesRegex(ValueError,'requires repair'):
                build_marine(path,[134.02,34.02,134.08,34.08],'fixture','https://source.test',p/'pack')
            self.assertFalse((p/'pack').exists())

    def test_orphan_between_multiple_exteriors_is_rejected(self):
        with tempfile.TemporaryDirectory() as root:
            p=Path(root);path=p/'water.shp'
            with shapefile.Writer(str(path),shapeType=shapefile.POLYGON) as writer:
                writer.field('id','N')
                writer.poly([
                    [[134,34],[134,34.02],[134.02,34.02],[134.02,34],[134,34]],
                    [[134.08,34.08],[134.08,34.1],[134.1,34.1],[134.1,34.08],[134.08,34.08]],
                    [[134.04,34.04],[134.06,34.04],[134.06,34.06],[134.04,34.06],[134.04,34.04]]]);writer.record(1)
            path.with_suffix('.prj').write_text(CRS.from_epsg(4326).to_wkt())
            with self.assertRaisesRegex(ValueError,'requires repair'):
                build_marine(path,[134,34,134.1,34.1],'fixture','https://source.test',p/'pack')
            self.assertFalse((p/'pack').exists())
