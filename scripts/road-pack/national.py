#!/usr/bin/env python3
"""Offline, disk-backed regional PBF -> sharded v1 road packs.

No HTTP, interpolation, simplification, or inference of missing coverage. One source
at a time; whole OSM ways are retained in SQLite, never the national graph in RAM.
"""
import argparse
import hashlib
import importlib.metadata
import json
import math
import os
from pathlib import Path
import shutil
import sqlite3
import tempfile
import build

LIMITS = dict(source_bytes=2*1024**3, source_nodes=200000000, ways=10000000,
              cells=300000, way_nodes=10000, cell_ways=10000, cell_coords=100000,
              cell_bytes=5*1024**2, output_bytes=12*1024**3, work_bytes=4*1024**3, min_free_bytes=3*1024**3)


def digest_file(path):
    digest=hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda:stream.read(1024*1024),b''): digest.update(chunk)
    return digest.hexdigest()


def read_poly(path):
    """Geofabrik poly union minus explicitly marked holes; reject invalid geometry."""
    from shapely.geometry import Polygon
    from shapely.ops import unary_union
    build.require(path.stat().st_size <= 1024*1024,'poly exceeds 1 MiB')
    rows=[r.strip() for r in path.read_text().splitlines() if r.strip()]
    build.require(3 <= len(rows) <= 20000,'invalid poly size')
    outers=[];holes=[];i=1
    while i<len(rows) and rows[i]!='END':
        label=rows[i];i+=1;points=[]
        while i<len(rows) and rows[i]!='END':
            points.append(build.point(tuple(map(float,rows[i].split()))));i+=1
        build.require(i<len(rows) and 4<=len(points)<=10000 and points[0]==points[-1], 'invalid poly ring')
        i+=1;p=Polygon(points)
        build.require(p.is_valid and not p.is_empty and p.area>0,'invalid poly geometry')
        (holes if label.startswith('!') else outers).append(p)
    build.require(i==len(rows)-1 and rows[i]=='END' and outers,'invalid poly ending')
    outer=unary_union(outers)
    for hole in holes: build.require(outer.covers(hole),'hole outside source polygon')
    result=outer.difference(unary_union(holes)) if holes else outer
    build.require(result.is_valid and not result.is_empty,'invalid source coverage')
    return result


def grid(n):
    x=n/build.STEP;r=round(x)
    return r if abs(x-r)<1e-9 else x


def cell_box(x,y):
    return tuple(round(v*build.STEP,10) for v in (x,y,x+1,y+1))


def certified_cells(geometry,limit):
    from shapely.geometry import box
    from shapely.prepared import prep
    prepared=prep(geometry);w,s,e,n=geometry.bounds
    # Restrict to the supported Japan play rectangle, not a claim about borders.
    w,s,e,n=max(w,122),max(s,20),min(e,154),min(n,46)
    build.require(w<e and s<n,'source outside supported play rectangle')
    cells=set();boundary=0
    for x in range(math.floor(grid(w)),math.ceil(grid(e))):
        for y in range(math.floor(grid(s)),math.ceil(grid(n))):
            b=box(*cell_box(x,y))
            if prepared.covers(b):
                cells.add((x,y));build.require(len(cells)<=limit,'certified cell limit')
            elif prepared.intersects(b): boundary+=1
    build.require(cells,'source has no wholly covered cells')
    return cells,boundary


def touched_cells(points,cells):
    """Segment traversal candidates in O(length), including both sides of boundaries."""
    result=set()
    for a,b in zip(points,points[1:]):
        steps=max(1,math.ceil(max(abs(b[0]-a[0]),abs(b[1]-a[1]))/build.STEP))
        build.require(steps<=20000,'unreasonable source segment span')
        for k in range(steps):
            u=[a[j]+(b[j]-a[j])*k/steps for j in (0,1)]
            v=[a[j]+(b[j]-a[j])*(k+1)/steps for j in (0,1)]
            for x in range(math.floor(grid(min(u[0],v[0])))-1,math.floor(grid(max(u[0],v[0])))+1):
                for y in range(math.floor(grid(min(u[1],v[1])))-1,math.floor(grid(max(u[1],v[1])))+1):
                    if (x,y) in cells and build.clipped(a,b,cell_box(x,y)) is not None:result.add((x,y))
    return result


def disk_guard(path,limits):
    build.require(shutil.disk_usage(path).free>=limits['min_free_bytes'],'insufficient free disk; nothing published')



class DiskBudget:
    """Allocated blocks for our immutable outputs plus the small mutable index dir.

    Never scan other jobs or repeatedly traverse all emitted cells. Each output
    file is statted once, its parent directory refreshed after a write; only the
    handful of SQLite/node-index files are rescanned at existing guard points.
    """
    def __init__(self,stage,limits):
        self.stage=stage;self.limits=limits;self.work=None
        self.file_bytes=0;self.directories={}
        self.refresh_directories(stage)
    @staticmethod
    def allocated(path):
        stat=path.stat()
        # Linux st_blocks uses 512-byte units, independently of st_blksize.
        return stat.st_blocks*512 if hasattr(stat,'st_blocks') else stat.st_size
    def refresh_directories(self,path):
        while True:
            self.directories[path]=self.allocated(path)
            if path==self.stage:break
            path=path.parent
    def used(self):
        mutable=0
        if self.work is not None and self.work.exists():
            mutable=self.allocated(self.work)+sum(self.allocated(p) for p in self.work.rglob('*'))
        return self.file_bytes+sum(self.directories.values())+mutable
    def check(self):
        disk_guard(self.stage,self.limits)
        build.require(self.used()<=self.limits['work_bytes'],'allocated work byte budget; nothing published')
    def write(self,path,data):
        self.check()
        with path.open('xb') as stream:stream.write(data)
        self.file_bytes+=self.allocated(path)
        self.refresh_directories(path.parent)
        self.check()


def extract(pbf,db,cells,temp,limits,budget=None):
    import osmium
    db.execute('CREATE TABLE ways(id TEXT PRIMARY KEY, payload BLOB NOT NULL)')
    db.execute('CREATE TABLE membership(x INTEGER,y INTEGER,id TEXT,PRIMARY KEY(x,y,id)) WITHOUT ROWID')
    stats={'nodes':0,'ways':0,'selected':0,'membership':0};db.execute('BEGIN')
    class Handler(osmium.SimpleHandler):
        def node(self,n):
            stats['nodes']+=1;build.require(stats['nodes']<=limits['source_nodes'],'source node limit')
            if stats['nodes']%1000000==0:budget.check() if budget else disk_guard(temp,limits)
        def way(self,w):
            stats['ways']+=1;build.require(stats['ways']<=limits['ways'],'source way limit')
            if w.tags.get('highway') not in build.CLASSES or w.tags.get('area')=='yes':return
            build.require(2<=len(w.nodes)<=limits['way_nodes'],'road node count outside budget')
            points=[];refs=[]
            for n in w.nodes:
                build.require(n.location.valid(),'missing road node; no coverage can be published')
                points.append(build.point((n.lon,n.lat)));refs.append(build.osm_id(str(n.ref)))
            touched=touched_cells(points,cells)
            if not touched:return
            tags={k:w.tags.get(k) for k in build.TAGS}
            build.require(all(v is None or len(v)<=4096 for v in tags.values()),'road tag too large')
            ident=build.osm_id(str(w.id));payload=build.encoded(dict(id=ident,version=str(w.version),node_ids=refs,coordinates=points,tags=tags))
            db.execute('INSERT INTO ways VALUES(?,?)',(ident,payload))
            db.executemany('INSERT INTO membership VALUES(?,?,?)',((x,y,ident) for x,y in touched))
            stats['selected']+=1;stats['membership']+=len(touched)
            if stats['selected']%256==0:budget.check() if budget else disk_guard(temp,limits)
            if stats['selected']%10000==0:
                db.commit();print(json.dumps({'extraction':stats}),flush=True);db.execute('BEGIN')
    Handler().apply_file(str(pbf),locations=True,idx=f'sparse_file_array,{temp}/nodes.idx')
    db.commit()
    return stats


def emit_source(stage,name,source,geometry,cells,boundary,db,limits,total,budget=None):
    from shapely.geometry import mapping
    shards={}
    for x,y in sorted(cells):
        shards.setdefault((x//50,y//50),[]).append((x,y))
    entries=[]
    for (sx,sy),subset in sorted(shards.items()):
        directory=stage/'packs'/name/f'{sx}_{sy}';directory.mkdir(parents=True)
        manifest=dict(format='rosenfu-road-pack',version=1,revision=source['timestamp'],cell_degrees=.02,
            source=dict(url=source['url'],format='osm-pbf',file_sha256=source['sha256'],parent_pbf_sha256=source['sha256'],
                coverage_geojson=mapping(geometry),coverage_sha256=build.sha(build.encoded(mapping(geometry))),poly_sha256=source['poly_sha256'],coverage_format='geojson',
                polygon_snapshot_match='unverified',coverage_verified=False,
                extraction='rosenfu-national-road-v1/whole-original-ways',parent_timestamp=source['timestamp']),
            requested_bounds=[sx,sy,sx+1,sy+1],highway_classes=build.CLASSES,excluded_area='yes',license='ODbL-1.0',
            license_url='https://opendatacommons.org/licenses/odbl/1-0/',attribution='© OpenStreetMap contributors',
            attribution_url='https://www.openstreetmap.org/copyright',
            coverage_meaning='Selected-road extraction from pinned regional source within supplied polygon. Polygon/PBF snapshot match unverified; NOT approved construction coverage. Missing cells are UNKNOWN, never empty land. Not real-world completeness.',cells={})
        for x,y in subset:
            records=[];coords=0
            for (raw,) in db.execute('SELECT payload FROM membership JOIN ways USING(id) WHERE x=? AND y=? ORDER BY length(id),id',(x,y)):
                way=json.loads(raw);records.append(way);coords+=len(way['node_ids'])
                build.require(len(records)<=limits['cell_ways'] and coords<=limits['cell_coords'],'cell exceeds runtime limits')
            key=f'{x}_{y}';bounds=cell_box(x,y)
            data=build.encoded(dict(format='rosenfu-road-cell',version=1,revision=source['timestamp'],bounds=bounds,ways=records))
            build.require(len(data)<=limits['cell_bytes'],'cell exceeds byte limit')
            total[0]+=len(data);build.require(total[0]<=limits['output_bytes'],'output byte budget')
            disk_guard(stage,limits)
            budget.write(directory/f'{key}.json',data) if budget else (directory/f'{key}.json').write_bytes(data)
            manifest['cells'][key]=dict(file=f'{key}.json',bounds=bounds,sha256=build.sha(data),bytes=len(data),ways=len(records))
        data=build.encoded(manifest);build.require(len(data)<=limits['cell_bytes'],'manifest too large')
        total[0]+=len(data);build.require(total[0]<=limits['output_bytes'],'output byte budget')
        budget.write(directory/'manifest.json',data) if budget else (directory/'manifest.json').write_bytes(data)
        entries.append(dict(source=name,bounds=[sx,sy,sx+1,sy+1],manifest=f'packs/{name}/{sx}_{sy}/manifest.json',sha256=build.sha(data),bytes=len(data),cells=len(subset)))
        disk_guard(stage,limits)
    return dict(name=name,source=source,certified_cells=len(cells),uncertified_boundary_cells=boundary,shards=entries)


def generate(config,output,limits=None):
    import osmium
    limits={**LIMITS,**(limits or {})}
    build.require(all(type(v) is int and v>0 for v in limits.values()),'positive budgets required')
    for package,version in [('osmium','4.3.2'),('shapely','2.1.2')]:build.require(importlib.metadata.version(package)==version,'unsupported '+package+' version')
    build.require(not output.exists(),'output already exists')
    build.require(1<=len(config)<=8,'one to eight named source regions required')
    names=set();sources=[];timestamp=None
    for row in config:
        name=row['name'];build.require(isinstance(name,str) and name.isascii() and name.replace('-','').isalnum() and name not in names,'invalid/duplicate source name');names.add(name)
        pbf=Path(row['pbf']);poly=Path(row['poly']);build.require(pbf.stat().st_size<=limits['source_bytes'],'source byte budget')
        build.require(row['url'].startswith('https://download.geofabrik.de/asia/japan/') and row['url'].endswith('.osm.pbf'),'expected Geofabrik Japan source URL')
        digest=digest_file(pbf);build.require(digest==row['sha256'],'source SHA256 mismatch')
        with osmium.io.Reader(str(pbf)) as reader:revision=reader.header().get('osmosis_replication_timestamp')
        build.require(revision and (timestamp is None or revision==timestamp),'source timestamps must match');timestamp=revision
        geometry=read_poly(poly);sources.append((name,pbf,poly,geometry,dict(url=row['url'],sha256=digest,poly_sha256=digest_file(poly),timestamp=revision)))
    output.parent.mkdir(parents=True,exist_ok=True);stage=Path(tempfile.mkdtemp(prefix='.national-roads-',dir=output.parent));total=[0];budget=DiskBudget(stage,limits)
    try:
        regions=[]
        for name,pbf,poly,geometry,source in sources:
            disk_guard(stage,limits);cells,boundary=certified_cells(geometry,limits['cells'])
            with tempfile.TemporaryDirectory(prefix='road-index-',dir=output.parent) as work:
                work=Path(work);budget.work=work
                with sqlite3.connect(work/'ways.sqlite') as db:
                    db.execute('PRAGMA cache_size=-16384');db.execute('PRAGMA temp_store=FILE')
                    stats=extract(pbf,db,cells,work,limits,budget)
                    # Osmium has closed its reader; SQLite now owns all retained
                    # coordinates. Do not keep the disk node index during emit.
                    (work/'nodes.idx').unlink(missing_ok=True)
                    region=emit_source(stage,name,source,geometry,cells,boundary,db,limits,total,budget);region['statistics']=stats;regions.append(region)
            budget.work=None;budget.check()
            print(json.dumps(dict(region=name,certified_cells=len(cells),boundary_unknown=boundary,output_bytes=total[0])),flush=True)
        index=dict(format='rosenfu-road-pack-index',version=1,revision=timestamp,license='ODbL-1.0',attribution='© OpenStreetMap contributors',
            attribution_url='https://www.openstreetmap.org/copyright',license_url='https://opendatacommons.org/licenses/odbl/1-0/',
            coverage_verified=False,polygon_snapshot_match='unverified',
            coverage='Polygon/PBF snapshot match unverified; NOT approved construction coverage. Only cells explicitly listed in authenticated shard manifests; all missing cells UNKNOWN. Regional boundaries may have gaps. No nationwide completeness claim.',
            regions=regions,output_bytes=total[0],generator=dict(osmium='4.3.2',shapely='2.1.2'),limits=limits)
        data=build.encoded(index);build.require(len(data)<=limits['cell_bytes'],'index size limit');budget.write(stage/'index.json',data)
        build.require(not output.exists(),'output appeared');os.rename(stage,output);return index
    finally:
        if stage.exists():shutil.rmtree(stage)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('sources',type=Path);parser.add_argument('output',type=Path)
    args=parser.parse_args();generate(json.loads(args.sources.read_text()),args.output)
