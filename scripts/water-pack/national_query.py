#!/usr/bin/env python3
"""Build selected query windows from all eight pinned regional stages and marine data."""
import argparse
import hashlib
import json
import re
from pathlib import Path
from build import check_bounds,encoded
from stage import extract_stage,pinned_json
from source_set import REGIONS
from marine import build_marine,source_files
from compose import compose_many
from delivery import build_delivery


def build_queries(config,output):
    output=Path(output)
    if output.exists():raise ValueError('New output directory required')
    stages=config['stages'];source_pin=config['source_set_sha256'];marine=config['marine'];cells=config['cells']
    if not isinstance(source_pin,str) or not re.fullmatch('[a-f0-9]{64}',source_pin):raise ValueError('Invalid source set pin')
    if not isinstance(stages,dict) or set(stages)!=REGIONS:raise ValueError('All eight regional stages required')
    timestamps=set()
    for region in sorted(REGIONS):
        entry=stages[region];m=pinned_json(Path(entry['directory'])/'stage-manifest.json',entry['sha256'])
        if m.get('region')!=region or m.get('source_set_sha256')!=source_pin or m.get('geographic_completeness')!='unknown' or m.get('coverage')!=[]:raise ValueError('Stage source set mismatch')
        timestamps.add(m['source']['osm_timestamp'])
    if len(timestamps)!=1:raise ValueError('Regional source timestamps differ')
    if not isinstance(cells,list) or not 1<=len(cells)<=32:raise ValueError('Select 1..32 query cells')
    keys=set()
    for cell in cells:
        key=cell['key'];check_bounds(cell['bounds'])
        if not isinstance(key,str) or not re.fullmatch('[a-z0-9][a-z0-9_-]{0,63}',key) or key in keys:raise ValueError('Invalid or duplicate cell key')
        keys.add(key)
    marine_path=Path(marine['path'])
    if hashlib.sha256(encoded(source_files(marine_path))).hexdigest()!=marine['bundle_sha256']:raise ValueError('Marine inventory pin mismatch')
    output.mkdir(parents=True,exist_ok=False);selection=[];leaf_bytes=0
    for cell in cells:
        cell_root=output/'work'/cell['key'];inputs=[]
        for region in sorted(REGIONS):
            entry=stages[region];key='inland_'+region;target=cell_root/key
            result=extract_stage(entry['directory'],entry['sha256'],cell['bounds'],target)
            inputs.append((key,target,result['manifest_sha256']))
        target=cell_root/'marine'
        result=build_marine(marine_path,cell['bounds'],marine['revision'],marine['url'],target)
        if result['source_bundle_sha256']!=marine['bundle_sha256']:raise ValueError('Marine inventory changed')
        inputs.append(('marine',target,result['manifest_sha256']))
        leaf_bytes+=sum(f.stat().st_size for _,directory,_ in inputs for f in directory.iterdir())
        if leaf_bytes>128*1024*1024:raise ValueError('Query batch byte limit; select fewer cells')
        target=cell_root/'composite';result=compose_many(inputs,target)
        selection.append(dict(key=cell['key'],directory=str(target),composite_sha256=result['composite_sha256']))
    report=build_delivery(selection,output/'release')
    return dict(**report,regional_timestamp=next(iter(timestamps)),marine_revision=marine['revision'],source_set_sha256=source_pin)


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('config',type=Path);parser.add_argument('--output',required=True);args=parser.parse_args()
    try:
        if not 0<args.config.stat().st_size<=1024*1024:raise ValueError('Config size limit')
        print(json.dumps(build_queries(json.loads(args.config.read_text()),args.output)))
    except (ValueError,OSError,KeyError,TypeError,ImportError) as error:parser.exit(1,f'national water query: {error}\n')
