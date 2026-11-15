#!/usr/bin/env python3
"""Bundle two independently pinned evidence packs, keeping their source revisions separate."""
import argparse
import hashlib
import json
from pathlib import Path
from build import MAX_BYTES,check_bounds,encoded
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


def compose(inland,inland_pin,marine,marine_pin,output):
    output=Path(output)
    if output.exists():raise ValueError('Output exists')
    parts=[('inland',inland_pin,read_part(inland,inland_pin)),('marine',marine_pin,read_part(marine,marine_pin))]
    if parts[0][2][0]['bounds']!=parts[1][2][0]['bounds']:raise ValueError('Requested part bounds differ')
    entries=[dict(key=key,manifest=f'{key}/manifest.json',payload=f'{key}/evidence.json',manifest_sha256=pin) for key,pin,_ in parts]
    raw=encoded(dict(format='rosenfu-water-composite',version=1,geographic_completeness='unknown',coverage=[],source_time_policy='retain-per-part-revisions',parts=entries))
    output.mkdir(parents=True,exist_ok=False)
    for key,_,(_,manifest,payload) in parts:
        (output/key).mkdir();(output/key/'manifest.json').write_bytes(manifest);(output/key/'evidence.json').write_bytes(payload)
    (output/'composite.json').write_bytes(raw)
    return dict(composite_sha256=hashlib.sha256(raw).hexdigest(),revisions={key:m['revision'] for key,_,(m,_,_) in parts},geographic_completeness='unknown')


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    for option in ('inland','inland-pin','marine','marine-pin','output'):parser.add_argument('--'+option,required=True)
    args=parser.parse_args()
    try:print(json.dumps(compose(args.inland,args.inland_pin,args.marine,args.marine_pin,args.output)))
    except (ValueError,OSError,KeyError,TypeError) as error:parser.exit(1,f'water composite: {error}\n')
