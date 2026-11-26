import {describe,it,expect,vi} from 'vitest';
import {createRoadRepository,parseRoadResponse,ROAD_LIMITS,roadBoundsKey,roadTileSnapshots,type RoadSnapshot} from './automaticRoadData';
const bounds={west:139,south:35,east:139.01,north:35.01};
const empty={osm3s:{timestamp_osm_base:'2026-11-15T00:00:00Z'},elements:[]};
const way={type:'way',id:10,nodes:[1,2],geometry:[{lat:35.004,lon:138.999},{lat:35.004,lon:139.011}],tags:{highway:'residential',layer:'1',bridge:'yes'}};
const response=(value:unknown)=>new Response(JSON.stringify(value),{status:200});
describe('automatic road reference acquisition',()=>{
 it('preserves source structures and complete way geometry outside requested coverage',async()=>{
  const data=await parseRoadResponse({...empty,elements:[way]},bounds,1);
  expect(data.bounds).toEqual(bounds);expect(data.bundle.graph.dataset.bounds.west).toBe(138.999);
  expect(data.bundle.graph.edges[0]).toMatchObject({layer:1,bridge:true,tunnel:null,level:null});
  expect(data.bundle.roadWays?.['10'].structureUnsupported).toBe(true);
 });
 it('distinguishes valid empty response from failed or partial queries',async()=>{
  expect((await parseRoadResponse(empty,bounds,1)).bundle.graph.edges).toEqual([]);
  await expect(parseRoadResponse({...empty,remark:'timeout'},bounds,1)).rejects.toThrow();
  await expect(parseRoadResponse({elements:[]},bounds,1)).rejects.toThrow();
 });
 it('rejects incomplete geometry and contradictory node coordinates',async()=>{
  await expect(parseRoadResponse({...empty,elements:[{...way,geometry:[]}]},bounds,1)).rejects.toThrow();
  await expect(parseRoadResponse({...empty,elements:[way,{...way,id:11,geometry:[{lat:36,lon:139},way.geometry[1]]}]},bounds,1)).rejects.toThrow();
 });
 it('does not turn malformed structural tags into supported ground',async()=>{
  const data=await parseRoadResponse({...empty,elements:[{...way,tags:{highway:'residential',layer:'unknown'}}]},bounds,1);
  expect(data.bundle.graph.edges[0].layer).toBeNull();expect(data.bundle.roadWays?.['10'].structureUnsupported).toBe(true);
 });
 it('coalesces concurrent queries and caches valid empty data',async()=>{
  const fetcher=vi.fn(async()=>response(empty)),repo=createRoadRepository(fetcher,()=>100);
  const [a,b]=await Promise.all([repo.load(bounds),repo.load(bounds)]);expect(a).toBe(b);
  await repo.load(bounds);expect(fetcher).toHaveBeenCalledTimes(1);
 });
 it('retains errors through cooldown then allows recovery',async()=>{
  let clock=0;const fetcher=vi.fn().mockRejectedValueOnce(Error('offline')).mockResolvedValueOnce(response(empty));const repo=createRoadRepository(fetcher,()=>clock);
  await expect(repo.load(bounds)).rejects.toThrow('offline');await expect(repo.load(bounds)).rejects.toThrow('offline');expect(fetcher).toHaveBeenCalledTimes(1);
  clock+=ROAD_LIMITS.retryMs;expect((await repo.load(bounds)).bundle.graph.edges).toEqual([]);
 });
 it('serializes different queries and rejects oversized request bounds',async()=>{
  let active=0,max=0;const fetcher=vi.fn(async()=>{max=Math.max(max,++active);await Promise.resolve();active--;return response(empty);});const repo=createRoadRepository(fetcher);
  await Promise.all([repo.load(bounds),repo.load({...bounds,east:139.02})]);expect(max).toBe(1);
  expect(()=>roadBoundsKey({...bounds,east:140})).toThrow();
 });
 it('rejects HTTP errors rather than treating them as empty roads',async()=>{
  const repo=createRoadRepository(vi.fn(async()=>new Response('',{status:429})));
  await expect(repo.load(bounds)).rejects.toThrow('429');
 });
});

it('tile selection retains the original evidence snapshot and excludes non-tile providers',async()=>{
 const overpass=await parseRoadResponse(empty,bounds,1);
 const tile:RoadSnapshot={...overpass,tileLines:[],coverageVerified:false};
 const selected=roadTileSnapshots({...overpass,tileSnapshots:[overpass,tile]});
 expect(selected).toHaveLength(1);
 expect(selected[0]).toBe(tile);
 expect(selected[0].bundle).toBe(tile.bundle);
 expect(selected[0].coverageVerified).toBe(false);
 expect(roadTileSnapshots(overpass)).toEqual([]);
 expect(roadTileSnapshots(undefined)).toEqual([]);
});
