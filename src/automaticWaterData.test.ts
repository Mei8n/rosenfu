import {afterEach,describe,it,expect,vi} from 'vitest';
import {PbfWriter} from 'pbf';
import {createWaterSource,createWaterRepository,decodeWaterTile,waterTileBounds} from './automaticWaterData';

afterEach(()=>{vi.useRealTimers();vi.restoreAllMocks();});
const x=14550,y=6460,z=14;
const inside=(dx=0)=>{const b=waterTileBounds(x+dx,y,z);return {west:b.west+0.001,east:b.east-0.001,south:b.south+0.001,north:b.north-0.001};};
// Synthetic MVT and PMTiles fixtures; no provider data or network needed in CI.
function mvt(name='WA',geometry?:number[],type=3){
 const p=new PbfWriter();p.writeMessage(3,(_,layer)=>{
  layer.writeStringField(1,name);layer.writeVarintField(5,4096);layer.writeVarintField(15,2);
  if(geometry)layer.writeMessage(2,(_,feature)=>{feature.writeVarintField(3,type);feature.writePackedVarint(4,geometry);},null);
 },null);return new Uint8Array(p.finish()).buffer;
}
const wet=mvt('WA',[9,0,0,26,200,0,0,200,199,0,15]);
function fixture(payload:ArrayBuffer=wet,available=true){
 const directory=new PbfWriter();for(const n of [1,0,available?2**30:1,payload.byteLength,1])directory.writeVarint(n);
 const root=directory.finish(),bytes=new Uint8Array(16384+payload.byteLength),view=new DataView(bytes.buffer);
 bytes.set(new TextEncoder().encode('PMTiles'));bytes[7]=3;
 for(const [offset,value] of [[8,127],[16,root.length],[56,16384],[64,payload.byteLength]])view.setBigUint64(offset,BigInt(value),true);
 bytes[97]=1;bytes[98]=1;bytes[99]=1;bytes[100]=14;bytes[101]=14;bytes.set(root,127);bytes.set(new Uint8Array(payload),16384);
 let etag='"v1"';
 const fetcher=vi.fn<typeof fetch>(async(_url,init)=>{
  const match=/bytes=(\d+)-(\d+)/.exec(new Headers(init?.headers).get('range')!)!;
  const start=Number(match[1]),end=Number(match[2]);
  return new Response(bytes.slice(start,end+1),{status:206,headers:{'content-range':`bytes ${start}-${end}/${bytes.length}`,etag}});
 });return {fetcher,setEtag:(value:string)=>{etag=value;}};
}

describe('bounded water source',()=>{
 it('cancels a server returning its whole archive',async()=>{
  const cancel=vi.fn(),body=new ReadableStream<Uint8Array>({cancel});
  const source=createWaterSource(vi.fn(async()=>new Response(body,{status:200})));
  await expect(source.getBytes(0,8)).rejects.toThrow('HTTP 200');expect(cancel).toHaveBeenCalledOnce();
 });
 it('requires matching content range, total size and body length',async()=>{
  for(const range of ['bytes 1-8/100','bytes 0-7/7','bytes 0-7/9007199254740992']){
   const source=createWaterSource(vi.fn(async()=>new Response(new Uint8Array(8),{status:206,headers:{'content-range':range}})));
   await expect(source.getBytes(0,8)).rejects.toThrow('Range');
  }
  const source=createWaterSource(vi.fn(async()=>new Response(new Uint8Array(7),{status:206,headers:{'content-range':'bytes 0-7/100'}})));
  await expect(source.getBytes(0,8)).rejects.toThrow('欠損');
 });
 it('rejects unsafe range sums and already cancelled requests before fetch',async()=>{
  const fetcher=vi.fn(),source=createWaterSource(fetcher);
  await expect(source.getBytes(Number.MAX_SAFE_INTEGER,3)).rejects.toThrow();
  await expect(source.getBytes(0,1e10)).rejects.toThrow();
  await expect(source.getBytes(0,8,AbortSignal.abort())).rejects.toThrow();expect(fetcher).not.toHaveBeenCalled();
 });
 it('propagates cancellation and timeout to fetch',async()=>{
  vi.useFakeTimers();const fetcher=vi.fn<typeof fetch>(async(_url,init)=>new Promise((_resolve,reject)=>{init!.signal!.addEventListener('abort',()=>reject(new DOMException('aborted','AbortError'))); }));
  const source=createWaterSource(fetcher),controller=new AbortController();
  const cancelled=expect(source.getBytes(0,8,controller.signal)).rejects.toThrow('aborted');controller.abort();await cancelled;
  const timeout=expect(source.getBytes(0,8)).rejects.toThrow('aborted');await vi.advanceTimersByTimeAsync(25000);await timeout;
 });
});

describe('water coverage evidence',()=>{
 it('distinguishes an explicit empty WA from absent WA',()=>{
  expect(decodeWaterTile(mvt(),x,y,z)).toEqual([]);
  for(const data of [new ArrayBuffer(0),mvt('AdmArea'),mvt('Cntr')])expect(()=>decodeWaterTile(data,x,y,z)).toThrow('カバレッジ');
 });
 it('rejects truncated and duplicate layers, non-polygons and empty polygon features',()=>{
  const empty=new Uint8Array(mvt()),duplicate=new Uint8Array(empty.length*2);duplicate.set(empty);duplicate.set(empty,empty.length);
  for(const data of [empty.slice(0,-1).buffer,duplicate.buffer,mvt('WA',[9,0,0],1),mvt('WA',[])])expect(()=>decodeWaterTile(data,x,y,z)).toThrow();
 });
 it('decodes water polygons and preserves holes',()=>{
  const water=decodeWaterTile(mvt('WA',[9,0,0,26,200,0,0,200,199,0,15,9,40,159,26,0,120,120,0,0,119,15]),x,y,z);
  expect(water).toHaveLength(1);expect(water[0].rings).toHaveLength(2);
  for(const ring of water[0].rings){expect(ring).toHaveLength(5);expect(ring[0]).toEqual(ring.at(-1));}
 });
 it('adjacent tile boundaries meet exactly',()=>{expect(waterTileBounds(x,y,z).east).toBe(waterTileBounds(x+1,y,z).west);});
});

describe('water repository recovery and subscribers',()=>{
 it('retries the network after a failed header, and waiting clients do not prolong cooldown',async()=>{
  vi.useFakeTimers();const f=fixture(),repo=createWaterRepository(f.fetcher);
  f.fetcher.mockRejectedValueOnce(Error('transient header failure'));
  await expect(repo.load(inside())).rejects.toThrow('transient');
  await vi.advanceTimersByTimeAsync(59000);await expect(repo.load(inside())).rejects.toThrow('待機');
  await vi.advanceTimersByTimeAsync(1000);
  const result=await repo.load(inside());expect(result.water).toHaveLength(1);expect(result.coverage).toHaveLength(1);expect(f.fetcher).toHaveBeenCalledTimes(3);
 });
 it.each(['etag','416'] as const)('invalidates old header and decoded tiles on archive change (%s)',async(mode)=>{
  vi.useFakeTimers();const f=fixture(),repo=createWaterRepository(f.fetcher);
  await repo.load(inside());f.setEtag('"v2"');
  if(mode==='416')f.fetcher.mockResolvedValueOnce(new Response(null,{status:416}));
  const changed=expect(repo.load(inside(1))).rejects.toThrow('更新');await vi.advanceTimersByTimeAsync(250);await changed;
  await vi.advanceTimersByTimeAsync(60000);await repo.load(inside());
  expect(f.fetcher).toHaveBeenCalledTimes(5); // old header/tile, changed tile, new header/tile
 });
 it('does not reuse old water polygons after the archive changes',async()=>{
  vi.useFakeTimers();const old=fixture(),updated=fixture(mvt());updated.setEtag('\"v2\"');
  const repo=createWaterRepository(old.fetcher);expect((await repo.load(inside())).water).toHaveLength(1);
  old.fetcher.mockImplementation(updated.fetcher);
  const changed=expect(repo.load(inside(1))).rejects.toThrow('更新');await vi.advanceTimersByTimeAsync(250);await changed;
  await vi.advanceTimersByTimeAsync(60000);expect((await repo.load(inside())).water).toEqual([]);
 });
 it('never certifies coverage for a tile absent from the archive index',async()=>{
  const f=fixture(wet,false);await expect(createWaterRepository(f.fetcher).load(inside())).rejects.toThrow('未提供');expect(f.fetcher).toHaveBeenCalledOnce();
 });
 it('coalesces loads and caches explicit dry coverage',async()=>{
  const f=fixture(mvt()),repo=createWaterRepository(f.fetcher);
  const [a,b]=await Promise.all([repo.load(inside()),repo.load(inside())]);
  expect(a).toEqual(b);expect(a.water).toEqual([]);expect(a.coverage).toEqual([waterTileBounds(x,y,z)]);
  await repo.load(inside());expect(f.fetcher).toHaveBeenCalledTimes(2);
 });
 it('expires decoded tiles after 24 hours',async()=>{
  vi.useFakeTimers();const f=fixture(),repo=createWaterRepository(f.fetcher);await repo.load(inside());
  await vi.advanceTimersByTimeAsync(86400000);await repo.load(inside());expect(f.fetcher).toHaveBeenCalledTimes(3);
 });
 it('cancels one subscriber without cancelling its peer or scheduling the next tile',async()=>{
  vi.useFakeTimers();const f=fixture(),original=f.fetcher.getMockImplementation()!;
  let release!:()=>void;const gate=new Promise<void>(resolve=>{release=resolve;});
  f.fetcher.mockImplementation(async(url,init)=>{if(new Headers(init?.headers).get('range')!.startsWith('bytes=16384-'))await gate;return original(url,init);});
  const repo=createWaterRepository(f.fetcher),controller=new AbortController(),bounds={...inside(),east:inside(1).east};
  const cancelled=expect(repo.load(bounds,controller.signal)).rejects.toThrow();const peer=repo.load(inside());
  await vi.advanceTimersByTimeAsync(0);controller.abort();await cancelled;release();expect((await peer).water).toHaveLength(1);
  await vi.advanceTimersByTimeAsync(1000);expect(f.fetcher).toHaveBeenCalledTimes(2);
  await repo.load(inside());expect(f.fetcher).toHaveBeenCalledTimes(2);
 });
 it('rejects oversized areas and pre-cancelled loads without fetching',async()=>{
  const fetcher=vi.fn(),repo=createWaterRepository(fetcher);
  await expect(repo.load({west:130,east:140,south:30,north:40})).rejects.toThrow();
  await expect(repo.load(inside(),AbortSignal.abort())).rejects.toThrow();expect(fetcher).not.toHaveBeenCalled();
 });
 it('never converts an omitted WA into cached dry coverage',async()=>{
  vi.useFakeTimers();const f=fixture(mvt('AdmArea')),repo=createWaterRepository(f.fetcher);
  await expect(repo.load(inside())).rejects.toThrow('カバレッジ');await vi.advanceTimersByTimeAsync(60000);
  await expect(repo.load(inside())).rejects.toThrow('カバレッジ');expect(f.fetcher).toHaveBeenCalledTimes(3);
 });
});
