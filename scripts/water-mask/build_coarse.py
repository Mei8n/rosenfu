#!/usr/bin/env python3
"""Build a scoped marine + large-lake map model, never geographic completeness.
The TWW1 container is global; cells outside manifest scope are UNSUPPORTED,
not dry. Consumers MUST enforce the associated catalog scope before lookup.
"""
import argparse,hashlib,json,math,struct
from pathlib import Path
from build import encoded,sha

def inventory(path):
    result=[]
    for ext in ['.shp','.shx','.dbf','.prj']:
        p=path.with_suffix(ext)
        if not p.is_file() or p.stat().st_size>2*1024**3:raise ValueError('Source component missing/too large')
        h=hashlib.sha256()
        with p.open('rb') as f:
            for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
        result.append(dict(file=p.name,bytes=p.stat().st_size,sha256=h.hexdigest()))
    return result

def source_shapes(path,bounds):
    import shapefile
    from shapely.geometry import shape,box
    from pyproj import CRS
    if not CRS.from_wkt(path.with_suffix('.prj').read_text()).equals(CRS.from_epsg(4326),ignore_axis_order=True):raise ValueError('Source is not WGS84')
    clip=box(*bounds);result=[];count=0
    with shapefile.Reader(str(path)) as r:
        if r.shapeType!=shapefile.POLYGON:raise ValueError('Source must be 2D polygons')
        for s in r.iterShapes(bbox=bounds):
            count+=len(s.points)
            if count>20_000_000 or len(result)>20000:raise ValueError('Source geometry limit')
            if any(not math.isfinite(x) or not math.isfinite(y) or abs(x)>180 or abs(y)>90 for x,y in s.points):raise ValueError('Invalid coordinates')
            for i,start in enumerate(s.parts):
                ring=s.points[start:s.parts[i+1] if i+1<len(s.parts) else len(s.points)]
                if len(ring)<4 or ring[0]!=ring[-1] or any(abs(a[0]-b[0])>180 for a,b in zip(ring,ring[1:])):raise ValueError('Unsupported ring')
            geo=s.__geo_interface__
            if s._errors:raise ValueError('Polygon conversion requires repair')
            g=shape(geo)
            if g.is_empty or not g.is_valid:raise ValueError('Invalid source polygon')
            g=g.intersection(clip)
            if not g.is_empty:result.append(g)
    return result,count

def rasterize(geometries,bounds,per=60):
    import numpy as np
    from shapely import STRtree,union_all,intersects_xy
    from shapely.geometry import box
    w,s,e,n=bounds
    if per!=60 or any(type(x)!=int for x in bounds) or not (-180<=w<e<=180 and -85<=s<n<=85) or (e-w)*(n-s)>1000:raise ValueError('Unsupported scope')
    width,height=(e-w)*per,(n-s)*per
    wet=np.zeros((height,width),dtype=bool);tree=STRtree(geometries)
    # Two-degree chunks keep sampling arrays small; 4x4 subcell centres per cell.
    for south in range(s,n,2):
        north=min(n,south+2)
        yy=south+(np.arange((north-south)*per*4)+.5)/(per*4)
        for west in range(w,e,2):
            east=min(e,west+2);ids=tree.query(box(west,south,east,north),predicate='intersects')
            if not len(ids):continue
            geom=union_all([geometries[int(i)] for i in ids])
            xx=west+(np.arange((east-west)*per*4)+.5)/(per*4)
            samples=intersects_xy(geom,xx[None,:],yy[:,None])
            counts=samples.reshape((north-south)*per,4,(east-west)*per,4).sum(axis=(1,3))
            wet[(south-s)*per:(north-s)*per,(west-w)*per:(east-w)*per]=counts>=8
        print('latitude band',south,north,flush=True)
    return wet

def encode_world(wet,bounds,per=60):
    import numpy as np
    w,s,e,n=bounds
    if wet.shape!=((n-s)*per,(e-w)*per) or per!=60:raise ValueError('Grid shape mismatch')
    out=bytearray(b'TWW1'+struct.pack('<III',360*per,180*per,per))
    def leb(value):
        while value>=128:out.append((value&127)|128);value>>=7
        out.append(value)
    for row in range(180*per):
        local=row-(s+90)*per
        if not 0<=local<len(wet):leb(0);continue
        edges=np.flatnonzero(np.diff(np.r_[False,wet[local],False]))+(w+180)*per
        leb(len(edges));previous=0
        for end in edges:leb(int(end)-previous);previous=int(end)
    if len(out)>16*1024*1024:raise ValueError('Output exceeds decoder limit')
    return bytes(out)

def build(marine,lakes,out,bounds,marine_pin,lakes_pin):
    if out.exists():raise ValueError('Output already exists')
    marine_files,lake_files=inventory(marine),inventory(lakes)
    if sha(encoded(marine_files))!=marine_pin or sha(encoded(lake_files))!=lakes_pin:raise ValueError('Source inventory hash mismatch')
    sea,sea_coords=source_shapes(marine,bounds);lake,lake_coords=source_shapes(lakes,bounds)
    print('source shapes',len(sea),len(lake),'coordinates',sea_coords+lake_coords,flush=True)
    wet=rasterize(sea+lake,bounds);data=encode_world(wet,bounds)
    if inventory(marine)!=marine_files or inventory(lakes)!=lake_files:raise ValueError('Source changed')
    policy='scoped-60-per-degree-4x4-centres-majority-v1'
    revision=sha(encoded(dict(policy=policy,marine=marine_pin,lakes=lakes_pin,bounds=bounds)))
    manifest=dict(format='rosenfu-scoped-coarse-water',version=1,revision=revision,scope=[bounds],outside_scope='unknown',geographic_completeness='not-guaranteed',policy=policy,per_degree=60,water_cells=int(wet.sum()),model_land_cells=int((~wet).sum()),marine_files=marine_files,lake_files=lake_files,payload=dict(file='japan-coarse.water',bytes=len(data),sha256=sha(data)),limitations=['Sea and large lakes only; small water and narrow streams can be absent','Different source dates; model classification is not a survey','Scope must be enforced before querying this global-format container'])
    out.mkdir(parents=True);(out/'japan-coarse.water').write_bytes(data);(out/'manifest.json').write_bytes(encoded(manifest)+b'\n')
    print(json.dumps({k:manifest[k] for k in ['revision','scope','water_cells','model_land_cells','payload']}),flush=True)
    return manifest

def catalog_for_manifest(manifest, requested_scope=None):
    # A scoped TWW1 container must never be published with a broader catalog.
    if manifest.get('format')!='rosenfu-scoped-coarse-water' or manifest.get('version')!=1 or manifest.get('outside_scope')!='unknown':raise ValueError('Invalid coarse manifest')
    declared=manifest['scope'];scope=declared if requested_scope is None else requested_scope
    for group in [declared,scope]:
        if not isinstance(group,list) or not 1<=len(group)<=1024:raise ValueError('Invalid scope')
        for b in group:
            if not isinstance(b,list) or len(b)!=4 or any(type(v) not in [int,float] or not math.isfinite(v) for v in b) or not (-180<=b[0]<b[2]<=180 and -85<=b[1]<b[3]<=85):raise ValueError('Invalid scope rectangle')
    if any(not any(d[0]<=b[0] and d[1]<=b[1] and b[2]<=d[2] and b[3]<=d[3] for d in declared) for b in scope):raise ValueError('Catalog exceeds generated scope')
    payload=manifest['payload'];revision=manifest['revision']
    if payload['file']!='japan-coarse.water' or not isinstance(revision,str) or len(revision)!=64:raise ValueError('Invalid generated descriptor')
    return dict(format='rosenfu-water-model-catalog',version=1,revision=revision,scope=json.loads(json.dumps(scope)),fineListing='complete',fine=[],coarse=dict(state='provided',sourceId='japan-coarse-'+revision,payload=dict(payload)))

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--marine',type=Path,required=True);p.add_argument('--lakes',type=Path,required=True);p.add_argument('--output',type=Path,required=True)
    p.add_argument('--bounds',nargs=4,type=int,default=[122,20,154,46]);p.add_argument('--marine-pin',required=True);p.add_argument('--lakes-pin',required=True)
    a=p.parse_args();build(a.marine,a.lakes,a.output,a.bounds,a.marine_pin,a.lakes_pin)
