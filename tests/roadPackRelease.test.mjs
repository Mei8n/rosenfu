import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {afterEach,expect,it,vi} from 'vitest';
import {createRoadPackRepository} from '../src/roadPackRepository';
import {ROAD_PACK_RELEASE} from '../src/roadPackRelease';
const root=new URL('../public/road-data/',import.meta.url);
const manifestBytes=readFileSync(new URL('manifest.json',root));
const manifest=JSON.parse(manifestBytes);
afterEach(()=>vi.unstubAllGlobals());
it('verifies every bundled real OSM cell through the production reader',async()=>{
 expect(createHash('sha256').update(manifestBytes).digest('hex')).toBe(ROAD_PACK_RELEASE.manifestSha256);
 vi.stubGlobal('location',new URL('https://road-test.invalid'));
 const fetcher=vi.fn(async input=>new Response(readFileSync(new URL(new URL(String(input)).pathname.split('/').at(-1),root))));
 const repo=createRoadPackRepository(new URL('https://road-test.invalid/road-data/'),ROAD_PACK_RELEASE.manifestSha256,fetcher);
 let ways=0;
 for(const cell of Object.values(manifest.cells)){
  const [w,s,e,n]=cell.bounds;const longitude=(w+e)/2,latitude=(s+n)/2;
  const snapshot=await repo.load({west:longitude-.0001,east:longitude+.0001,south:latitude-.0001,north:latitude+.0001});
  ways+=snapshot.roads.length;
  expect(snapshot.bundle.sourceSha256).toBe(manifest.source.parent_pbf_sha256);
  expect(snapshot.bundle.source.license).toBe('ODbL-1.0');
  expect(snapshot.bundle.graph.dataset.revision).toBe(manifest.revision);
 }
 expect(ways).toBeGreaterThan(1000);
 expect(fetcher).toHaveBeenCalledTimes(Object.keys(manifest.cells).length+1);
 // No additional public-service request or empty coverage outside the release.
 const count=fetcher.mock.calls.length;
 await expect(repo.load({west:140,south:36,east:140.001,north:36.001})).rejects.toThrow(/未提供/);
 expect(fetcher).toHaveBeenCalledTimes(count);
},30000);
