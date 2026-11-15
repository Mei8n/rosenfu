"""Pinned official-source producer. Never publishes releases or certifies coverage."""
import argparse
import hashlib
import json
import io
import zipfile
from pathlib import Path
import re
import shutil
import sys
import tempfile
import time
import urllib.request

HERE=Path(__file__).resolve().parent
sys.path[:0]=[str(HERE.parent/'road-pack'),str(HERE.parent/'road-delivery')]
import package

NAMES=('hokkaido','tohoku','kanto','chubu','kansai','chugoku','shikoku','kyushu')
SHA=re.compile('[a-f0-9]{64}')
BASE='https://download.geofabrik.de/asia/japan/'
def require(ok,message):
    if not ok:raise ValueError(message)
def encoded(value):return (json.dumps(value,ensure_ascii=False,sort_keys=True,separators=(',',':'))+'\n').encode()
def source_catalog():
    data=json.loads((HERE/'sources.json').read_bytes())
    require(data['version']==1 and data['coverage_verified'] is False and data['polygon_snapshot_match']=='unverified','source policy')
    require([r['name'] for r in data['regions']]==list(NAMES),'source region order')
    for r in data['regions']:
        require(r['pbf_url']==BASE+r['name']+'-261114.osm.pbf' and r['poly_url']==BASE+r['name']+'.poly','fixed source URL')
        for kind,maximum in [('pbf',2*1024**3),('poly',1024**2)]:
            require(SHA.fullmatch(r[kind+'_sha256']) and type(r[kind+'_bytes']) is int and 0<r[kind+'_bytes']<=maximum,'source hash/size')
    return data
class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self,*args,**kwargs):raise ValueError('source redirect rejected')
def download(url,destination,sha,size,opener=None):
    require(url.startswith(BASE) and not any(c in url for c in '?#@'),'official fixed URL only')
    require(not destination.exists(),'source already exists')
    opener=opener or urllib.request.build_opener(NoRedirect())
    temporary=destination.with_suffix(destination.suffix+'.partial')
    require(not temporary.exists(),'partial source already exists')
    try:
        digest=hashlib.sha256();received=0;deadline=time.monotonic()+900
        request=urllib.request.Request(url,headers={'User-Agent':'Rosenfu-road-producer/1','Accept-Encoding':'identity'})
        with opener.open(request,timeout=30) as response, temporary.open('xb') as stream:
            require(response.status==200 and response.geturl()==url,'unexpected source response')
            for block in iter(lambda:response.read(1024*1024),b''):
                require(time.monotonic()<deadline,'source download deadline')
                received+=len(block);require(received<=size,'source size exceeded')
                require(shutil.disk_usage(destination.parent).free>=3*1024**3,'source disk reserve')
                digest.update(block);stream.write(block)
        require(received==size and digest.hexdigest()==sha,'source hash/size mismatch')
        temporary.rename(destination)
    finally:
        temporary.unlink(missing_ok=True)
def produce(region,output):
    import national
    config=source_catalog();source=next(r for r in config['regions'] if r['name']==region)
    require(not output.exists(),'output exists')
    output.parent.mkdir(parents=True,exist_ok=True)
    require(shutil.disk_usage(output.parent).free>=8*1024**3,'producer requires 8 GiB free')
    with tempfile.TemporaryDirectory(prefix='road-producer-',dir=output.parent) as temp:
        work=Path(temp);pbf=work/'source.osm.pbf';poly=work/'source.poly'
        for kind,path in [('pbf',pbf),('poly',poly)]:download(source[kind+'_url'],path,source[kind+'_sha256'],source[kind+'_bytes'])
        raw=work/'raw'
        result=national.generate([dict(name=region,pbf=str(pbf),poly=str(poly),url=source['pbf_url'],sha256=source['pbf_sha256'])],raw,dict(cells=500000,ways=20000000,work_bytes=4*1024**3,output_bytes=2*1024**3,min_free_bytes=3*1024**3))
        require(result['revision']==config['revision'] and result['coverage_verified'] is False,'source revision/coverage changed')
        package.package(raw,work/'release')
        (work/'release').rename(output)

def combine(inputs,output,regions):
    """Merge complete authenticated per-region packages without expanding archives."""
    config=source_catalog();expected={r['name']:r for r in config['regions']}
    require(regions and len(regions)==len(set(regions)) and set(regions)<=set(NAMES),'requested regions')
    require(not output.exists(),'output exists')
    output.parent.mkdir(parents=True,exist_ok=True)
    stage=Path(tempfile.mkdtemp(prefix='.road-combined-',dir=output.parent))
    total=0;assets=[];allregions=[];index=None;catalog=None
    try:
        for name in regions:
            root=inputs/name
            require(not root.is_symlink(),'symlink regional input')
            raw=package.read(root/'index.json');part=json.loads(raw);cat=json.loads(package.read(root/'catalog.json'))
            require(cat.get('format')=='rosenfu-road-release' and cat.get('version')==2 and cat.get('publishable') is True,'package contract')
            require(cat.get('index_sha256')==package.digest(raw),'index hash')
            require(part.get('format')=='rosenfu-road-pack-index' and part.get('version')==1 and part.get('revision')==config['revision'] and cat.get('revision')==config['revision'],'index revision')
            require(part.get('coverage_verified') is False and part.get('polygon_snapshot_match')=='unverified' and cat.get('coverage_verified') is False,'coverage must remain unverified')
            require(part.get('license')=='ODbL-1.0' and part.get('attribution') and cat.get('license')==part['license'] and cat.get('attribution')==part['attribution'],'data attribution')
            require(len(part['regions'])==1 and part['regions'][0]['name']==name,'region identity')
            r=part['regions'][0];s=r['source'];pin=expected[name]
            require(s['url']==pin['pbf_url'] and s['sha256']==pin['pbf_sha256'] and s['poly_sha256']==pin['poly_sha256'] and s['timestamp']==config['revision'],'source identity')
            shards={a['manifest']:a for a in r['shards']}
            require(len(shards)==len(r['shards']) and shards,'shard count')
            seen=set(); ranges={}; manifests={}
            for a in cat['assets']:
                require(re.fullmatch(re.escape(name)+r'-\d+_\d+-p\d{3}\.zip',a['name']) and a['manifest'] in shards and a['name'] not in seen,'asset path/duplicate')
                seen.add(a['name']);shard=shards[a['manifest']]
                require(a['manifest_sha256']==shard['sha256'] and type(a['cell_count']) is int and 0<a['cell_count']<=shard['cells'] and a['files']==a['cell_count']+1 and 0<a['expanded_bytes']<=package.MAX_EXPANDED,'shard contract')
                data=package.read(root/a['name'],package.MAX_COMPRESSED)
                require(len(data)==a['bytes'] and package.digest(data)==a['sha256'],'asset integrity')
                with zipfile.ZipFile(io.BytesIO(data)) as archive:
                    info=archive.getinfo(a['manifest'])
                    require(info.file_size==shard['bytes'] and info.file_size<=package.MAX_FILE,'manifest size')
                    manifest=archive.read(info)
                    require(package.digest(manifest)==shard['sha256'],'manifest integrity')
                    cells=json.loads(manifest)['cells'];keys=sorted(cells)
                    require(len(keys)==shard['cells'] and a['first_cell'] in cells and a['last_cell'] in cells,'range endpoints')
                    first,last=keys.index(a['first_cell']),keys.index(a['last_cell'])
                    require(last>=first and last-first+1==a['cell_count'],'range size')
                    expected_files={a['manifest']}|{a['manifest'].rsplit('/',1)[0]+'/'+cells[k]['file'] for k in keys[first:last+1]}
                    require(len(archive.infolist())==a['files'] and set(archive.namelist())==expected_files,'part files')
                    require(sum(i.file_size for i in archive.infolist())==a['expanded_bytes'],'expanded size')
                    ranges.setdefault(a['manifest'],[]).append((first,last))
                total+=len(data);require(total<=package.MAX_TOTAL_COMPRESSED and len(assets)<package.MAX_ASSETS,'combined release budget')
                require(shutil.disk_usage(stage).free>=package.MIN_FREE+len(data),'combined disk reserve')
                (stage/a['name']).write_bytes(data);assets.append(a)
            require(set(ranges)==set(shards),'missing shard')
            for manifest, spans in ranges.items():
                cursor=0
                for first,last in sorted(spans):
                    require(first==cursor,'part gap or overlap');cursor=last+1
                require(cursor==shards[manifest]['cells'],'incomplete shard')
            allregions.append(r);index=part;catalog=cat
        index={**index,'regions':allregions}
        # Generated totals are informational; recompute from authenticated archive declarations.
        index['output_bytes']=sum(a['expanded_bytes'] for a in assets)
        raw=encoded(index);require(len(raw)<=package.MAX_FILE,'combined index size');(stage/'index.json').write_bytes(raw)
        catalog={**catalog,'assets':assets,'index_sha256':package.digest(raw),'coverage_verified':False}
        data=encoded(catalog);require(len(data)<=package.MAX_FILE,'combined catalog size');(stage/'catalog.json').write_bytes(data)
        stage.rename(output)
        return catalog
    finally:
        if stage.exists():shutil.rmtree(stage)
if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);sub=parser.add_subparsers(dest='command',required=True)
    p=sub.add_parser('produce');p.add_argument('region',choices=NAMES);p.add_argument('output',type=Path)
    p=sub.add_parser('combine');p.add_argument('input',type=Path);p.add_argument('output',type=Path);p.add_argument('regions',nargs='+',choices=NAMES)
    args=parser.parse_args()
    if args.command=='produce':produce(args.region,args.output)
    else:combine(args.input,args.output,args.regions)
