#!/usr/bin/env python3
"""Build a bounded static release of verified trial composites, never coverage."""
import argparse
import hashlib
import json
import re
from pathlib import Path
from build import encoded
from compose import compose_many


def build_delivery(entries,output):
    output=Path(output)
    if output.exists() or not isinstance(entries,list) or not 0<len(entries)<=256:
        raise ValueError('New output and 1..256 selected cells required')
    names=set();pins=set();prepared=[]
    for entry in entries:
        name,pin=entry['key'],entry['composite_sha256']
        if not isinstance(name,str) or not re.fullmatch('[a-z0-9][a-z0-9_-]{0,63}',name) or name in names:raise ValueError('Invalid or duplicate cell key')
        if not isinstance(pin,str) or not re.fullmatch('[a-f0-9]{64}',pin) or pin in pins:raise ValueError('Invalid or duplicate composite pin')
        names.add(name);pins.add(pin);directory=Path(entry['directory']);path=directory/'composite.json'
        if not path.is_file() or not 0<path.stat().st_size<=65536:raise ValueError('Invalid composite size')
        raw=path.read_bytes()
        if hashlib.sha256(raw).hexdigest()!=pin:raise ValueError('Composite pin mismatch')
        index=json.loads(raw)
        if index.get('format')!='rosenfu-water-composite' or index.get('version')!=1 or index.get('geographic_completeness')!='unknown' or index.get('coverage')!=[] or index.get('source_time_policy')!='retain-per-part-revisions':raise ValueError('Unsupported composite')
        parts=index.get('parts')
        if not isinstance(parts,list) or not 2<=len(parts)<=16:raise ValueError('Require 2..16 parts')
        keys=set()
        for part in parts:
            key=part.get('key')
            if not isinstance(key,str) or not re.fullmatch('[a-z][a-z0-9_-]{0,31}',key) or key in keys or part.get('manifest')!=key+'/manifest.json' or part.get('payload')!=key+'/evidence.json':raise ValueError('Invalid part path')
            keys.add(key)
        prepared.append((name,pin,directory,parts))
    output.mkdir(parents=True,exist_ok=False)
    cells=[];total=0
    for name,pin,directory,parts in prepared:
        target=output/pin
        result=compose_many([(part['key'],directory/part['key'],part['manifest_sha256']) for part in parts],target)
        # Canonical reconstruction must retain the caller's exact composite pin.
        if result['composite_sha256']!=pin:raise ValueError('Composite reconstruction mismatch')
        m=json.loads((target/parts[0]['manifest']).read_text())
        total+=sum(f.stat().st_size for f in target.rglob('*') if f.is_file())
        if total>512*1024*1024:raise ValueError('Trial delivery size limit')
        cells.append(dict(key=name,directory=pin+'/',composite_sha256=pin,bounds=m['bounds'],revisions=result['revisions'],geographic_completeness='unknown',coverage=[]))
    raw=encoded(dict(format='rosenfu-water-delivery',version=1,geographic_completeness='unknown',coverage=[],cells=cells,license_policy='retain-per-part-source-metadata',limitations=['Selected trial cells only, not nationwide coverage','Keep per-part source provenance and revisions','No dry-land certification; width and relation completeness unresolved']))
    # This marker is written last; partial directories are not releases.
    (output/'delivery.json').write_bytes(raw)
    return dict(delivery_sha256=hashlib.sha256(raw).hexdigest(),cells=len(cells),bytes=total,geographic_completeness='unknown')


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('selection',type=Path);parser.add_argument('--output',required=True)
    args=parser.parse_args()
    try:
        if not 0<args.selection.stat().st_size<=1024*1024:raise ValueError('Selection size limit')
        print(json.dumps(build_delivery(json.loads(args.selection.read_text()),args.output)))
    except (ValueError,OSError,KeyError,TypeError) as error:parser.exit(1,f'water delivery: {error}\n')
