import io
import zipfile
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import produce

class Producer(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup);self.root=Path(self.tmp.name)
    def fixture(self,name='kanto'):
        pins=produce.source_catalog();pin=next(r for r in pins['regions'] if r['name']==name)
        raw=self.root/('raw-'+name);directory=raw/'packs'/name/'139_35';directory.mkdir(parents=True)
        cell=b'{"ways":[]}';(directory/'6985_1780.json').write_bytes(cell)
        manifest=produce.encoded({'cells':{'6985_1780':{'file':'6985_1780.json','bytes':len(cell),'sha256':produce.package.digest(cell)}}})
        (directory/'manifest.json').write_bytes(manifest)
        index={'format':'rosenfu-road-pack-index','version':1,'revision':pins['revision'],'license':'ODbL-1.0','attribution':'OSM contributors','coverage_verified':False,'polygon_snapshot_match':'unverified','regions':[{'name':name,'source':{'url':pin['pbf_url'],'sha256':pin['pbf_sha256'],'poly_sha256':pin['poly_sha256'],'timestamp':pins['revision']},'shards':[{'manifest':f'packs/{name}/139_35/manifest.json','sha256':produce.package.digest(manifest),'bytes':len(manifest),'cells':1}]}]}
        (raw/'index.json').write_bytes(produce.encoded(index));dest=self.root/'inputs'/name
        produce.package.package(raw,dest);return dest
    def mutate_index(self,directory,mutate):
        index=json.loads((directory/'index.json').read_bytes());mutate(index);raw=produce.encoded(index);(directory/'index.json').write_bytes(raw)
        catalog=json.loads((directory/'catalog.json').read_bytes());catalog['index_sha256']=produce.package.digest(raw);(directory/'catalog.json').write_bytes(produce.encoded(catalog))
    def combine(self,names):return produce.combine(self.root/'inputs',self.root/'out',names)
    def test_pinned_catalog_has_eight_regions(self):
        self.assertEqual(len(produce.source_catalog()['regions']),8)
    def test_combine_two_regions_keeps_coverage_unknown(self):
        self.fixture();self.fixture('shikoku');result=self.combine(['kanto','shikoku'])
        self.assertTrue(result['publishable']);self.assertFalse(result['coverage_verified']);self.assertEqual(len(result['assets']),2)
        index=json.loads((self.root/'out/index.json').read_bytes());self.assertEqual([r['name'] for r in index['regions']],['kanto','shikoku'])
    def test_missing_region_does_not_publish_partial(self):
        self.fixture()
        with self.assertRaises((ValueError,FileNotFoundError)):self.combine(['kanto','shikoku'])
        self.assertFalse((self.root/'out').exists());self.assertFalse(list(self.root.glob('.road-combined-*')))
    def test_part_metadata_gap_is_rejected(self):
        d=self.fixture();path=d/'catalog.json';cat=json.loads(path.read_bytes())
        cat['assets'][0]['first_cell']='9999_9999';path.write_bytes(produce.encoded(cat))
        with self.assertRaisesRegex(ValueError,'range endpoints'):self.combine(['kanto'])
        self.assertFalse((self.root/'out').exists())
    def test_duplicate_part_is_rejected(self):
        d=self.fixture();path=d/'catalog.json';cat=json.loads(path.read_bytes());cat['assets']*=2;path.write_bytes(produce.encoded(cat))
        with self.assertRaisesRegex(ValueError,'duplicate'):self.combine(['kanto'])
    def test_v1_is_not_accepted(self):
        d=self.fixture();path=d/'catalog.json';cat=json.loads(path.read_bytes());cat['version']=1;path.write_bytes(produce.encoded(cat))
        with self.assertRaisesRegex(ValueError,'package contract'):self.combine(['kanto'])
    def mutate_zip(self,mode):
        d=self.fixture();cp=d/'catalog.json';cat=json.loads(cp.read_bytes());a=cat['assets'][0];p=d/a['name']
        with zipfile.ZipFile(p) as z: entries=[(i,z.read(i)) for i in z.infolist()]
        with zipfile.ZipFile(p,'w') as z:
            for info,data in entries:
                if not info.filename.endswith('manifest.json'):
                    if mode=='content':data=data.replace(b'ways',b'oops')
                    if mode=='symlink':info.external_attr=0o120777<<16
                    if mode=='size':data+=b' '
                z.writestr(info,data)
        blob=p.read_bytes();a.update(bytes=len(blob),sha256=produce.package.digest(blob))
        if mode=='size':a['expanded_bytes']+=1
        cp.write_bytes(produce.encoded(cat))
    def test_cell_rehashed_archive_cannot_hide_tampering(self):
        self.mutate_zip('content')
        with self.assertRaisesRegex(ValueError,'member integrity'):self.combine(['kanto'])
        self.assertFalse((self.root/'out').exists())
    def test_symlink_member_rejected(self):
        self.mutate_zip('symlink')
        with self.assertRaisesRegex(ValueError,'non-regular'):self.combine(['kanto'])
    def test_cell_size_rejected(self):
        self.mutate_zip('size')
        with self.assertRaisesRegex(ValueError,'member size'):self.combine(['kanto'])
    def test_source_identity_conflict(self):
        d=self.fixture();self.mutate_index(d,lambda i:i['regions'][0]['source'].update(sha256='f'*64))
        with self.assertRaisesRegex(ValueError,'source identity'):self.combine(['kanto'])
    def test_no_promotion_of_coverage(self):
        d=self.fixture();self.mutate_index(d,lambda i:i.update(coverage_verified=True))
        with self.assertRaisesRegex(ValueError,'coverage'):self.combine(['kanto'])
    def test_archive_corruption(self):
        d=self.fixture();next(d.glob('*.zip')).write_bytes(b'corrupt')
        with self.assertRaisesRegex(ValueError,'integrity'):self.combine(['kanto'])
    def test_path_and_duplicate_region_rejected(self):
        self.fixture()
        for names in [['kanto','kanto'],['../kanto'],[]]:
            with self.assertRaisesRegex(ValueError,'requested'):self.combine(names)
    def test_combined_budget_is_atomic(self):
        self.fixture()
        with patch.object(produce.package,'MAX_TOTAL_COMPRESSED',1):
            with self.assertRaisesRegex(ValueError,'budget'):self.combine(['kanto'])
        self.assertFalse((self.root/'out').exists())
    def download(self,body=b'ok',sha=None,size=2,status=200,url=None):
        url=url or produce.BASE+'kanto.poly'
        class Response(io.BytesIO):
            def geturl(self):return url
        response=Response(body);response.status=status
        class Opener:
            def open(self,*args,**kwargs):return response
        with patch.object(produce.shutil,'disk_usage',return_value=type('U',(),{'free':10*1024**3})()):
            produce.download(produce.BASE+'kanto.poly',self.root/'download',sha or produce.package.digest(body),size,Opener())
    def test_download_exact_hash_and_size(self):
        self.download();self.assertEqual((self.root/'download').read_bytes(),b'ok')
    def test_bad_download_has_no_partial_file(self):
        for args in [{'sha':'f'*64},{'size':1},{'status':206},{'url':'https://other.test'}]:
            with self.assertRaises(ValueError):self.download(**args)
            self.assertFalse((self.root/'download').exists());self.assertFalse((self.root/'download.partial').exists())
    def test_redirect_rejected(self):
        with self.assertRaisesRegex(ValueError,'redirect'):produce.NoRedirect().redirect_request(None,None,None,None,None,None)
if __name__=='__main__':unittest.main()
