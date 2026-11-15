import contextlib
import importlib.util
import io
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest
from unittest.mock import patch
import national
import build

AVAILABLE=importlib.util.find_spec('osmium') and importlib.util.find_spec('shapely')

@unittest.skipUnless(AVAILABLE,'pinned optional osmium/shapely required')
class NationalTests(unittest.TestCase):
    def setUp(self):
        import osmium
        self.osmium=osmium;self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name);self.pbf=self.root/'source.osm.pbf';self.poly=self.root/'source.poly';self.out=self.root/'out'
        self.poly.write_text('source\n1\n139.70 35.60\n139.74 35.60\n139.74 35.64\n139.70 35.64\n139.70 35.60\nEND\nEND\n')
    def write(self,path=None,revision='2026-11-14T21:20:41Z',missing=False):
        path=path or self.pbf;header=self.osmium.io.Header();header.set('osmosis_replication_timestamp',revision)
        with self.osmium.SimpleWriter(str(path),header=header) as writer:
            writer.add_node(self.osmium.osm.mutable.Node(id=1,version=1,location=(139.69,35.61)))
            if not missing:writer.add_node(self.osmium.osm.mutable.Node(id=2,version=1,location=(139.75,35.61)))
            writer.add_way(self.osmium.osm.mutable.Way(id=90071992547409933,version=2,nodes=[1,2],tags={'highway':'residential','bridge':'no','layer':'0'}))
            writer.add_way(self.osmium.osm.mutable.Way(id=20,version=1,nodes=[1,2],tags={'highway':'service','area':'yes'}))
    def source(self,name='fixture',pbf=None):
        pbf=pbf or self.pbf
        return dict(name=name,pbf=str(pbf),poly=str(self.poly),url='https://download.geofabrik.de/asia/japan/'+name+'-261114.osm.pbf',sha256=national.digest_file(pbf))
    def run_pack(self,rows=None,limits=None):
        with contextlib.redirect_stdout(io.StringIO()):return national.generate(rows or [self.source()],self.out,{"min_free_bytes":1,**(limits or {})})
    def test_complete_empty_and_road_cells_with_hashes_and_original_geometry(self):
        self.write();index=self.run_pack();region=index['regions'][0]
        self.assertFalse(index['coverage_verified']);self.assertEqual(index['polygon_snapshot_match'],'unverified');self.assertEqual(region['certified_cells'],4);self.assertEqual(region['uncertified_boundary_cells'],0)
        shard=region['shards'][0];raw=(self.out/shard['manifest']).read_bytes();self.assertEqual(build.sha(raw),shard['sha256'])
        manifest=json.loads(raw);self.assertEqual(manifest['source']['format'],'osm-pbf');self.assertNotIn('xml_sha256',manifest['source'])
        self.assertEqual(build.sha(build.encoded(manifest['source']['coverage_geojson'])),manifest['source']['coverage_sha256'])
        populated=empty=0
        for key,entry in manifest['cells'].items():
            data=(self.out/Path(shard['manifest']).parent/entry['file']).read_bytes()
            self.assertEqual(build.sha(data),entry['sha256']);self.assertEqual(len(data),entry['bytes'])
            cell=json.loads(data)
            if cell['ways']:
                populated+=1;self.assertEqual(len(cell['ways']),1);way=cell['ways'][0]
                self.assertEqual(way['id'],'90071992547409933');self.assertEqual(way['coordinates'],[[139.69,35.61],[139.75,35.61]])
                self.assertEqual(way['node_ids'],['1','2']);self.assertEqual(way['tags']['bridge'],'no')
            else:empty+=1
        self.assertEqual((populated,empty),(2,2))
    def test_node_index_released_before_cell_emission(self):
        self.write();original=national.emit_source
        def inspect(*args,**kwargs):
            self.assertFalse(list(self.root.glob('road-index-*/nodes.idx')))
            return original(*args,**kwargs)
        with patch.object(national,'emit_source',side_effect=inspect):self.run_pack()
    def test_allocated_budget_counts_only_own_files_and_mutable_indexes(self):
        stage=self.root/'stage';stage.mkdir();work=self.root/'work';work.mkdir()
        budget=national.DiskBudget(stage,{**national.LIMITS,'min_free_bytes':1})
        budget.work=work
        before=budget.used();(self.root/'unrelated-cache').write_bytes(b'x'*65536)
        self.assertEqual(budget.used(),before)
        # A tiny output still occupies a filesystem block, not one logical byte.
        budget.write(stage/'cell.json',b'x')
        expected=sum(national.DiskBudget.allocated(p) for p in [stage,stage/'cell.json',work])
        self.assertEqual(budget.used(),expected)
        (work/'nodes.idx').write_bytes(b'x'*8192)
        self.assertEqual(budget.used(),expected+national.DiskBudget.allocated(work/'nodes.idx'))
        budget.limits={**budget.limits,'work_bytes':budget.used()-1}
        with self.assertRaisesRegex(ValueError,'allocated work'):budget.check()
    def test_global_free_floor_still_applies_independently(self):
        from collections import namedtuple
        stage=self.root/'stage';stage.mkdir()
        budget=national.DiskBudget(stage,{**national.LIMITS,'min_free_bytes':3000})
        usage=namedtuple('Usage','total used free')
        with patch.object(national.shutil,'disk_usage',return_value=usage(10000,7001,2999)):
            with self.assertRaisesRegex(ValueError,'insufficient free'):budget.check()
    def test_work_budget_failure_does_not_publish(self):
        self.write()
        with self.assertRaisesRegex(ValueError,'allocated work'):self.run_pack(limits={'work_bytes':1})
        self.assertFalse(self.out.exists());self.assertFalse(list(self.root.glob('.national-roads-*')))
        self.assertFalse(list(self.root.glob('road-index-*')))
    def test_region_boundary_cells_unknown_and_holes_never_certified(self):
        from shapely.geometry import Polygon
        geometry=Polygon([(139.70,35.60),(139.745,35.60),(139.745,35.645),(139.70,35.645)])
        cells,boundary=national.certified_cells(geometry,100)
        self.assertEqual(len(cells),4);self.assertEqual(boundary,5)
        hole=self.poly.read_text().replace('END\nEND\n','END\n!hole\n139.705 35.605\n139.715 35.605\n139.715 35.615\n139.705 35.615\n139.705 35.605\nEND\nEND\n')
        self.poly.write_text(hole);cells,_=national.certified_cells(national.read_poly(self.poly),100)
        self.assertNotIn((6985,1780),cells)
    def test_segment_on_boundary_is_in_both_adjacent_cells(self):
        cells={(6985,1780),(6986,1780)}
        self.assertEqual(national.touched_cells([(139.72,35.605),(139.72,35.615)],cells),cells)
    def test_mixed_snapshot_rejected_before_publication(self):
        self.write();other=self.root/'other.pbf';self.write(other,revision='2026-11-15T21:20:41Z')
        with self.assertRaisesRegex(ValueError,'timestamps'):self.run_pack([self.source(),self.source('other',other)])
        self.assertFalse(self.out.exists())
    def test_hash_mismatch_rejected(self):
        self.write();row=self.source();row['sha256']='0'*64
        with self.assertRaisesRegex(ValueError,'SHA256'):self.run_pack([row])
        self.assertFalse(self.out.exists())
    def test_missing_nodes_fail_without_publishing(self):
        self.write(missing=True)
        with self.assertRaises((ValueError,RuntimeError)):self.run_pack()
        self.assertFalse(self.out.exists());self.assertFalse(list(self.root.glob('.national-roads-*')))
    def test_small_output_budget_is_atomic(self):
        self.write()
        with self.assertRaisesRegex(ValueError,'output byte budget'):self.run_pack(limits={'output_bytes':1})
        self.assertFalse(self.out.exists());self.assertFalse(list(self.root.glob('.national-roads-*')))
    def test_cell_limits_cannot_silently_truncate(self):
        self.write()
        with self.assertRaisesRegex(ValueError,'cell exceeds runtime'):self.run_pack(limits={'cell_coords':1})
        self.assertFalse(self.out.exists())
    def test_invalid_poly_not_repaired_or_certified(self):
        self.poly.write_text('x\n1\n139.70 35.60\n139.74 35.64\n139.74 35.60\n139.70 35.64\n139.70 35.60\nEND\nEND\n')
        with self.assertRaisesRegex(ValueError,'invalid poly'):national.read_poly(self.poly)

if __name__=='__main__':unittest.main()
