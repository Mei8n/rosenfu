#!/usr/bin/env python3
"""Bundle two independently pinned evidence packs, keeping their source revisions separate."""
import argparse
import hashlib
import json
import re
from pathlib import Path
from build import MAX_BYTES,check_bounds,encoded,geometry_points
from validate_evidence import validate_leaf


def read_part(directory,pin):
    directory=Path(directory);values=[]
    for name in ('manifest.json','evidence.json'):
        path=directory/name
        if not path.is_file() or not 0<path.stat().st_size<=MAX_BYTES:raise ValueError('Part missing or oversized')
        values.append(path.read_bytes())
    manifest,payload=values
    if hashlib.sha256(manifest).hexdigest()!=pin:raise ValueError('Part manifest pin mismatch')
    m,d=json.loads(manifest.decode('utf-8')),json.loads(payload.decode('utf-8'))
    validate_leaf(m,d)
    if m.get('format')!='rosenfu-water-evidence-manifest' or d.get('format')!='rosenfu-water-evidence' or m.get('version')!=1 or d.get('version')!=1:raise ValueError('Unsupported part')
    for document in (m,d):
        if document.get('geographic_completeness')!='unknown' or document.get('coverage')!=[]:raise ValueError('Composite cannot certify coverage')
        check_bounds(document.get('bounds'))
    if m['bounds']!=d['bounds'] or m.get('revision')!=d.get('revision') or m['payload'].get('file')!='evidence.json' or m['payload'].get('bytes')!=len(payload) or m['payload'].get('sha256')!=hashlib.sha256(payload).hexdigest():raise ValueError('Part payload mismatch')
    return m,manifest,payload


def compose_many(inputs,output):
    output=Path(output)
    if output.exists():raise ValueError('Output exists')
    if not isinstance(inputs,list) or not 2<=len(inputs)<=16:raise ValueError('Require 2..16 independently pinned parts')
    seen=set();parts=[];total=0
    for key,directory,pin in inputs:
        if not isinstance(key,str) or not re.fullmatch('[a-z][a-z0-9_-]{0,31}',key) or key in seen:raise ValueError('Invalid or duplicate part key')
        seen.add(key);part=read_part(directory,pin);total+=len(part[1])+len(part[2])
        if total>20*1024*1024:raise ValueError('Composite byte limit')
        parts.append((key,pin,part))
    if any(p[2][0]['bounds']!=parts[0][2][0]['bounds'] for p in parts):raise ValueError('Requested part bounds differ')
    observations=sum(len(json.loads(p[2][2])['observations']) for p in parts)
    coordinates=sum(len(geometry_points(o['geometry'])) for p in parts for o in json.loads(p[2][2])['observations'])
    if observations>20000 or coordinates>200000:raise ValueError('Composite geometry limit')
    entries=[dict(key=key,manifest=f'{key}/manifest.json',payload=f'{key}/evidence.json',manifest_sha256=pin) for key,pin,_ in parts]
    raw=encoded(dict(format='rosenfu-water-composite',version=1,geographic_completeness='unknown',coverage=[],source_time_policy='retain-per-part-revisions',parts=entries))
    output.mkdir(parents=True,exist_ok=False)
    for key,_,(_,manifest,payload) in parts:
        (output/key).mkdir();(output/key/'manifest.json').write_bytes(manifest);(output/key/'evidence.json').write_bytes(payload)
    (output/'composite.json').write_bytes(raw)
    return dict(composite_sha256=hashlib.sha256(raw).hexdigest(),revisions={key:m['revision'] for key,_,(m,_,_) in parts},geographic_completeness='unknown')


def compose(inland,inland_pin,marine,marine_pin,output):
    return compose_many([('inland',inland,inland_pin),('marine',marine,marine_pin)],output)


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    for option in ('inland','inland-pin','marine','marine-pin','output'):parser.add_argument('--'+option,required=True)
    args=parser.parse_args()
    try:print(json.dumps(compose(args.inland,args.inland_pin,args.marine,args.marine_pin,args.output)))
    except (ValueError,OSError,KeyError,TypeError) as error:parser.exit(1,f'water composite: {error}\n')
