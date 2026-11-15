#!/usr/bin/env python3
"""Stage a whole regional OSM snapshot on disk; extract bounded evidence, never coverage."""
import argparse
import datetime
import re
import hashlib
import json
import sqlite3
from contextlib import closing
from pathlib import Path
from build import Collector, MAX_BYTES, WATER_SELECTION, digest_file, encoded, geometry_points, pack_bytes, read_osm
from source_set import REGIONS

MAX_STAGE_BYTES=512*1024*1024


def pinned_json(path,pin,limit=1024*1024):
    path=Path(path)
    if not path.is_file() or not 0<path.stat().st_size<=limit:raise ValueError('Manifest size limit')
    raw=path.read_bytes()
    if hashlib.sha256(raw).hexdigest()!=pin:raise ValueError('Manifest pin mismatch')
    return json.loads(raw)


def source_entry(source,region,timestamp):
    if not isinstance(timestamp,str):raise ValueError('Invalid source timestamp')
    date=datetime.datetime.strptime(timestamp,'%Y-%m-%dT%H:%M:%SZ')
    if date.strftime('%Y-%m-%dT%H:%M:%SZ')!=timestamp:raise ValueError('Invalid source timestamp')
    if region not in REGIONS or not isinstance(source,dict) or source.get('region')!=region or source.get('osm_timestamp')!=timestamp:raise ValueError('Source identity mismatch')
    if source.get('url')!=f'https://download.geofabrik.de/asia/japan/{region}-{date:%y%m%d}.osm.pbf':raise ValueError('Invalid source URL')
    if not isinstance(source.get('sha256'),str) or not re.fullmatch('[a-f0-9]{64}',source['sha256']):raise ValueError('Invalid source hash')
    if type(source.get('bytes')) is not int or not 0<source['bytes']<=768*1024*1024:raise ValueError('Invalid source size')
    if not isinstance(source.get('generator'),str) or len(source['generator'])>4096:raise ValueError('Invalid source generator')


class DiskCollector:
    def __init__(self,connection):
        self.connection=connection;self.issues=[];self.source_header={}
        self.count=self.coordinates=self.bytes=0

    def add(self,identity,kind,geometry,tags):
        points=geometry_points(geometry)
        x,y=points[0]
        # Reuse the leaf validator on one full observation, without retaining the region.
        check=Collector([max(-180,x-.001),max(-85,y-.001),min(180,x+.001),min(85,y+.001)])
        check.add(identity,kind,geometry,tags)
        item=check.items[0];body=encoded(item)
        self.count+=1;self.coordinates+=len(points);self.bytes+=len(body)
        if self.count>500000 or self.coordinates>12000000 or self.bytes>384*1024*1024 or len(body)>MAX_BYTES:
            raise ValueError('Regional stage limit exceeded')
        w,e=min(p[0] for p in points),max(p[0] for p in points)
        s,n=min(p[1] for p in points),max(p[1] for p in points)
        self.connection.execute('INSERT INTO observations(kind,source_id,west,south,east,north,body) VALUES(?,?,?,?,?,?,?)',(kind,identity,w,s,e,n,body))
        if self.count%1000==0:self.connection.commit()


def stage_region(path,source_set,source_set_pin,region,output):
    path,output=Path(path),Path(output)
    if output.exists() or region not in REGIONS:raise ValueError('New stage directory and known region required')
    source_set=pinned_json(source_set,source_set_pin)
    if source_set.get('format')!='rosenfu-water-source-set' or source_set.get('version')!=1 or source_set.get('geographic_completeness')!='unknown' or source_set.get('coverage')!=[]:raise ValueError('Unsupported source set')
    entries=source_set.get('sources')
    if not isinstance(entries,list) or len(entries)!=8 or {e.get('region') for e in entries}!=REGIONS:raise ValueError('Eight distinct source entries required')
    if source_set.get('license')!='ODbL-1.0' or source_set.get('license_url')!='https://opendatacommons.org/licenses/odbl/1-0/':raise ValueError('Unsupported source license')
    for entry in entries:source_entry(entry,entry['region'],source_set.get('osm_timestamp'))
    source=next(e for e in entries if e['region']==region)
    if not path.is_file() or path.suffix!='.pbf' or not 0<path.stat().st_size<=768*1024*1024 or path.stat().st_size!=source['bytes'] or digest_file(path)!=source['sha256']:raise ValueError('Regional source mismatch')
    output.mkdir(parents=True,exist_ok=False)
    db=output/'stage.sqlite'
    location_cache=output/'locations.idx'
    if ',' in str(location_cache.resolve()):raise ValueError('Location cache path cannot contain comma')
    with closing(sqlite3.connect(db)) as connection:
        connection.execute('PRAGMA temp_store=MEMORY')
        connection.execute('CREATE TABLE observations(kind TEXT NOT NULL, source_id TEXT NOT NULL, west REAL NOT NULL, south REAL NOT NULL, east REAL NOT NULL, north REAL NOT NULL, body BLOB NOT NULL, UNIQUE(kind,source_id))')
        collector=DiskCollector(connection)
        read_osm(path,collector,location_index='sparse_file_array,'+str(location_cache.resolve()))
        location_cache.unlink(missing_ok=True)
        if collector.source_header.get('osmosis_replication_timestamp')!=source_set['osm_timestamp'] or source.get('osm_timestamp')!=source_set['osm_timestamp']:raise ValueError('Stage timestamp mismatch')
        if path.stat().st_size!=source['bytes'] or digest_file(path)!=source['sha256']:raise ValueError('Source changed during staging')
        connection.execute('CREATE INDEX observation_bounds ON observations(west,east,south,north)')
        connection.commit()
        if connection.execute('PRAGMA integrity_check').fetchone()[0]!='ok':raise ValueError('Stage integrity check failed')
    if db.stat().st_size>MAX_STAGE_BYTES:raise ValueError('Stage database size limit')
    manifest=dict(format='rosenfu-water-stage',version=1,region=region,geographic_completeness='unknown',coverage=[],source_set_sha256=source_set_pin,source=source,source_header=collector.source_header,
        database=dict(file='stage.sqlite',bytes=db.stat().st_size,sha256=digest_file(db),observations=collector.count,coordinates=collector.coordinates),source_issues=collector.issues,
        limitations=['All selected source observations scanned, not all real-world water','Unassembled relations can be missing without callbacks','Empty spatial queries remain unknown'])
    raw=encoded(manifest);(output/'stage-manifest.json').write_bytes(raw)
    return dict(stage_manifest_sha256=hashlib.sha256(raw).hexdigest(),observations=collector.count,coordinates=collector.coordinates,source_issues=len(collector.issues),geographic_completeness='unknown')


def extract_stage(stage,manifest_pin,bounds,output):
    stage,output=Path(stage),Path(output)
    if output.exists():raise ValueError('Output exists')
    m=pinned_json(stage/'stage-manifest.json',manifest_pin)
    if m.get('format')!='rosenfu-water-stage' or m.get('version')!=1 or m.get('geographic_completeness')!='unknown' or m.get('coverage')!=[] or m['database']['file']!='stage.sqlite':raise ValueError('Unsupported stage')
    source_entry(m['source'],m.get('region'),m['source'].get('osm_timestamp'))
    if m['source_header'].get('osmosis_replication_timestamp')!=m['source']['osm_timestamp']:raise ValueError('Stage source timestamp mismatch')
    db=stage/'stage.sqlite';expected=m['database']
    if not db.is_file() or not 0<db.stat().st_size<=MAX_STAGE_BYTES or db.stat().st_size!=expected['bytes'] or digest_file(db)!=expected['sha256']:raise ValueError('Stage database mismatch')
    collector=Collector(bounds);w,s,e,n=bounds
    with closing(sqlite3.connect(db.resolve().as_uri()+'?mode=ro',uri=True)) as connection:
        connection.execute('PRAGMA trusted_schema=OFF')
        connection.execute('PRAGMA temp_store=MEMORY')
        for (body,) in connection.execute('SELECT body FROM observations WHERE west<=? AND east>=? AND south<=? AND north>=? ORDER BY kind,source_id',(e,w,n,s)):
            if len(body)>MAX_BYTES:raise ValueError('Stage observation size limit')
            item=json.loads(body);collector.add(item['id'],item['kind'],item['geometry'],item['tags'])
    if db.stat().st_size!=expected['bytes'] or digest_file(db)!=expected['sha256']:raise ValueError('Stage changed during extraction')
    if not isinstance(m['source_issues'],list) or len(m['source_issues'])>1000:raise ValueError('Stage issue limit')
    for issue in m['source_issues']:
        if not isinstance(issue,dict) or any(not isinstance(issue.get(key),str) or not 0<len(issue[key])<=limit for key,limit in [('id',256),('reason',4096)]):raise ValueError('Invalid stage issue')
    collector.issues=m['source_issues']
    original=m['source']
    source=dict(url=original['url'],file_sha256=original['sha256'],bytes=original['bytes'],header=m['source_header'],license='ODbL-1.0',license_url='https://opendatacommons.org/licenses/odbl/1-0/',attribution='© OpenStreetMap contributors; extracted by Rosenfu',selection=WATER_SELECTION+'; waterway=* lines; natural=coastline lines',stage_manifest_sha256=manifest_pin)
    raw,payload,manifest=pack_bytes(collector,original['osm_timestamp'],source)
    output.mkdir(parents=True,exist_ok=False)
    (output/'evidence.json').write_bytes(payload);(output/'manifest.json').write_bytes(raw)
    return dict(manifest_sha256=hashlib.sha256(raw).hexdigest(),counts=manifest['payload']['counts'],source_issues=len(collector.issues),geographic_completeness='unknown')


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);commands=parser.add_subparsers(dest='command',required=True)
    stage=commands.add_parser('stage');stage.add_argument('input',type=Path)
    for name in ('source-set','source-set-pin','region','output'):stage.add_argument('--'+name,required=True)
    extract=commands.add_parser('extract');extract.add_argument('stage',type=Path);extract.add_argument('--pin',required=True);extract.add_argument('--bounds',nargs=4,type=float,required=True);extract.add_argument('--output',required=True)
    args=parser.parse_args()
    try:
        result=stage_region(args.input,args.source_set,args.source_set_pin,args.region,args.output) if args.command=='stage' else extract_stage(args.stage,args.pin,args.bounds,args.output)
        print(json.dumps(result))
    except (ValueError,OSError,KeyError,TypeError,ImportError,sqlite3.Error) as error:parser.exit(1,f'water stage: {error}\n')
