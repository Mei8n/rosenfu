#!/usr/bin/env python3
"""Verify eight Geofabrik regional inputs share an OSM timestamp, not water coverage."""
import argparse
import hashlib
import json
from pathlib import Path
from build import digest_file, encoded

REGIONS = {'hokkaido','tohoku','kanto','chubu','kansai','chugoku','shikoku','kyushu'}


def audit_sources(receipts, expected_timestamp):
    import datetime
    import osmium
    timestamp = datetime.datetime.strptime(expected_timestamp,'%Y-%m-%dT%H:%M:%SZ')
    if not isinstance(receipts,list) or len(receipts)!=len(REGIONS):
        raise ValueError('All eight distinct regional receipts required')
    seen=set(); entries=[]; total=0
    for receipt in receipts:
        region=receipt.get('region')
        if region not in REGIONS or region in seen or receipt.get('status')!='verified-transfer':
            raise ValueError('Missing, duplicate or unverified regional input')
        seen.add(region)
        filename=f'{region}-{timestamp:%y%m%d}.osm.pbf'
        url='https://download.geofabrik.de/asia/japan/'+filename
        path=Path(receipt['path'])
        if receipt.get('source_url')!=url or path.name!=filename or not path.is_file():
            raise ValueError('Input URL/name does not match expected dated regional source')
        size=path.stat().st_size;total+=size
        if not 0<size<=768*1024*1024 or total>3*1024**3 or size!=receipt.get('bytes'):
            raise ValueError('Source size mismatch or limit')
        expected_hash=receipt.get('sha256')
        if not isinstance(expected_hash,str) or len(expected_hash)!=64 or digest_file(path)!=expected_hash:
            raise ValueError('Source hash mismatch')
        with osmium.io.Reader(str(path)) as reader:
            header=reader.header()
            actual=header.get('osmosis_replication_timestamp')
            generator=header.get('generator')
        if path.stat().st_size!=size or digest_file(path)!=expected_hash:
            raise ValueError('Source changed during header verification')
        if actual!=expected_timestamp:
            raise ValueError(f'{region}: missing or inconsistent OSM source timestamp')
        entries.append(dict(region=region,url=url,bytes=size,sha256=expected_hash,osm_timestamp=actual,generator=generator))
    return dict(format='rosenfu-water-source-set',version=1,geographic_completeness='unknown',coverage=[],
        osm_timestamp=expected_timestamp,source_snapshot_consistency='verified-input-headers',sources=sorted(entries,key=lambda e:e['region']),
        license='ODbL-1.0',license_url='https://opendatacommons.org/licenses/odbl/1-0/',attribution='© OpenStreetMap contributors',
        limitations=['Byte hashes and matching source timestamps do not establish real-world completeness','Regional borders, missing relations and waterway widths remain unresolved','Marine data has independent provenance and is not included in this snapshot claim'])


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('receipts',type=Path)
    parser.add_argument('--timestamp',required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    try:
        if args.receipts.stat().st_size>1024*1024: raise ValueError('Receipt size limit')
        data=encoded(audit_sources(json.loads(args.receipts.read_text()),args.timestamp))
        with args.output.open('xb') as stream: stream.write(data)
        print(json.dumps(dict(source_set_sha256=hashlib.sha256(data).hexdigest(),geographic_completeness='unknown')))
    except (ValueError,OSError,KeyError,TypeError,ImportError) as error:
        parser.exit(1,f'water sources: {error}\n')
