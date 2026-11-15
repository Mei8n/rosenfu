#!/usr/bin/env python3
"""Optional operator pipeline: fixed local PBF + matching .poly -> bounded road pack.
Install pyosmium==4.3.2 separately. No network client, rendering tiles or code copying.
"""
import argparse
import hashlib
import json
import os
import importlib.metadata
import math
from pathlib import Path
import tempfile
import xml.etree.ElementTree as ET
import build


def poly_geojson(path, boxes):
    build.require(path.stat().st_size<=1024*1024,'coverage file exceeds 1 MiB')
    lines=[line.strip() for line in path.read_text().splitlines() if line.strip()]
    build.require(3<=len(lines)<=10000,'invalid .poly size')
    rings=[];i=1
    while i<len(lines) and lines[i]!='END':
        name=lines[i];i+=1;ring=[]
        while i<len(lines) and lines[i]!='END':
            ring.append(list(map(float,lines[i].split())));i+=1
        build.require(i<len(lines),'unterminated polygon')
        i+=1
        if name.startswith('!'):
            raise ValueError('source .poly holes are unsupported; no coverage is certified')
        else:
            value={'type':'Polygon','coordinates':[ring]}
            rings.append({'name':name,'value':value,'ring':build.polygon(value),'holes':False})
    build.require(i==len(lines)-1 and lines[i]=='END','trailing polygon data')
    for candidate in rings:
        if not candidate['holes'] and all(build.covered(box,candidate['ring']) for box in boxes):
            return {'type':'Feature','properties':{'source_poly_ring':candidate['name']},'geometry':candidate['value']}
    raise ValueError('no single hole-free source polygon wholly covers requested cells')


def generate(pbf, poly, bounds, origin, output, max_pbf_bytes=256*1024**2, max_source_nodes=20000000):
    import osmium
    osmium_version=importlib.metadata.version("osmium")
    build.require(osmium_version=="4.3.2", "use validated pyosmium 4.3.2")
    build.require(not output.exists(),"output must not exist")
    build.require(0<max_pbf_bytes<=2*1024**3 and 0<max_source_nodes<=200000000,'operator budget outside supported limits')
    build.require(pbf.stat().st_size <= max_pbf_bytes,'PBF exceeds operator input limit')
    build.require(len(bounds)==4 and all(math.isfinite(x) for x in bounds), 'invalid bounds')
    build.require(0 < bounds[2]-bounds[0] <= .5 and 0 < bounds[3]-bounds[1] <= .5,'choose a bounded region no larger than .5 degrees')
    # Use exactly the cells the emitter will certify, including conservative
    # extra cells caused by binary floating-point boundaries.
    boxes=[box for _,box in build.grid_cells(bounds)]
    region=[min(b[0] for b in boxes),min(b[1] for b in boxes),max(b[2] for b in boxes),max(b[3] for b in boxes)]
    coverage=poly_geojson(poly,boxes)
    reader=osmium.io.Reader(str(pbf)); revision=reader.header().get('osmosis_replication_timestamp');reader.close()
    build.require(bool(revision),'source snapshot timestamp is missing')
    nodes={}; ways=[]; counters={"nodes":0,"refs":0}
    class Extract(osmium.SimpleHandler):
        def node(self,n):
            counters['nodes']+=1
            build.require(counters['nodes']<=max_source_nodes,'source node cache limit')
        def way(self,w):
            if w.tags.get('highway') not in build.CLASSES or w.tags.get('area')=='yes': return
            points=[];refs=[]
            for n in w.nodes:
                build.require(n.location.valid(),'source way has missing coordinates')
                points.append((n.lon,n.lat));refs.append(str(n.ref))
            if not any(build.clipped(a,b,region) is not None for a,b in zip(points,points[1:])): return
            build.require(len(ways)<100000,'selected way limit')
            for ident,point in zip(refs,points):
                build.require(ident not in nodes or nodes[ident]==point,'conflicting original node')
                nodes[ident]=point
                build.require(len(nodes)<=1000000,'selected node limit')
            counters['refs']+=len(refs)
            build.require(counters['refs']<=1000000,'selected node reference limit')
            ways.append((str(w.id),str(w.version),refs,{k:w.tags.get(k) for k in build.TAGS}))
    output.parent.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='road-node-index-',dir=output.parent) as cache:
        Extract().apply_file(str(pbf),locations=True,idx=f'sparse_file_array,{cache}/nodes.idx')
    root=ET.Element('osm',{'version':'0.6','generator':'rosenfu-road-pack-v1'})
    for ident,(lon,lat) in sorted(nodes.items(),key=lambda p:int(p[0])):
        ET.SubElement(root,'node',{'id':ident,'lon':str(lon),'lat':str(lat)})
    for ident,version,refs,tags in ways:
        w=ET.SubElement(root,'way',{'id':ident,'version':version})
        for ref in refs: ET.SubElement(w,'nd',{'ref':ref})
        for k,v in tags.items():
            if v is not None: ET.SubElement(w,'tag',{'k':k,'v':v})
    output.parent.mkdir(parents=True,exist_ok=True)
    digest=hashlib.sha256()
    with pbf.open('rb') as stream:
        for chunk in iter(lambda:stream.read(1024*1024),b''):digest.update(chunk)
    with tempfile.TemporaryDirectory(prefix='road-input-',dir=output.parent) as temp:
        xml=Path(temp)/'selected.osm'; geo=Path(temp)/'coverage.geojson'; stage=Path(temp)/'pack'
        ET.ElementTree(root).write(xml,encoding='utf-8',xml_declaration=True)
        geo.write_bytes(build.encoded(coverage))
        manifest=build.generate(xml,geo,bounds,revision,origin,stage)
        manifest['source'].update(parent_pbf_sha256=digest.hexdigest(),poly_sha256=build.sha(poly.read_bytes()),
            parent_timestamp=revision,extraction='rosenfu-road-pack-v1/whole-ways-intersecting-emitted-grid-bounds',
            extraction_bounds=region,pyosmium=osmium_version)
        (stage/'manifest.json').write_bytes(build.encoded(manifest))
        build.require(not output.exists(),'output appeared during generation')
        os.rename(stage,output)
    print(json.dumps({'revision':revision,'selected_ways':len(ways),'source_nodes':counters['nodes'],'nodes':len(nodes),'cells':len(manifest['cells']),
        'parent_pbf_sha256':digest.hexdigest(),'manifest_sha256':build.sha((output/'manifest.json').read_bytes())}))



if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('pbf',type=Path);p.add_argument('--poly',type=Path,required=True)
    p.add_argument('--bounds',type=float,nargs=4,required=True);p.add_argument('--source-url',required=True);p.add_argument('--output',type=Path,required=True)
    p.add_argument('--max-pbf-bytes',type=int,default=256*1024**2);p.add_argument('--max-source-nodes',type=int,default=20000000)
    a=p.parse_args();generate(a.pbf,a.poly,a.bounds,a.source_url,a.output,a.max_pbf_bytes,a.max_source_nodes)
