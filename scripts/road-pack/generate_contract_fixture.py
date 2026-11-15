"""Regenerate the tiny synthetic PBF-to-TypeScript contract fixture. No real source certification."""
import json
from pathlib import Path
import shutil
import test_national
import build

def main():
    case=test_national.NationalTests();case.setUp()
    try:
        case.write();index=case.run_pack()
        # This polygon and source were created together in this test, not inferred from real data.
        index['coverage_verified']=True;index['polygon_snapshot_match']='verified'
        for region in index['regions']:
            for shard in region['shards']:
                path=case.out/shard['manifest'];manifest=json.loads(path.read_bytes())
                manifest['source']['coverage_verified']=True
                manifest['source']['polygon_snapshot_match']='verified'
                data=build.encoded(manifest);path.write_bytes(data)
                shard['sha256']=build.sha(data);shard['bytes']=len(data)
        index['output_bytes']=sum(p.stat().st_size for p in case.out.rglob('*') if p.is_file() and p.name!='index.json')
        (case.out/'index.json').write_bytes(build.encoded(index))
        dest=Path(__file__).parent/'fixtures'/'generated-national'
        if dest.exists():shutil.rmtree(dest)
        shutil.copytree(case.out,dest)
    finally:case.doCleanups()

if __name__=='__main__':main()
