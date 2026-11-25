// @vitest-environment jsdom
import {act,cleanup,renderHook} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
const mocks=vi.hoisted(()=>({load:vi.fn(),read:vi.fn(),pinned:vi.fn(),observe:vi.fn()}));
vi.mock('./roadProvider',()=>({createRoadProvider:()=>({name:'fixture',load:mocks.load,readTileEvidenceBatch:mocks.read,readPinnedSeamTiles:mocks.pinned,observeTiles:mocks.observe})}));
import {useAutomaticRoad} from './useAutomaticRoad';
const bounds={west:139.70,east:139.72,south:35.62,north:35.64};
const snapshot={roads:[],tileSnapshots:[{revision:'fixed'}],bundle:{}};
const evidence={rawTiles:[],bindings:[]};
afterEach(()=>{cleanup();vi.useRealTimers();vi.clearAllMocks();});
async function setup(){vi.useFakeTimers();mocks.load.mockResolvedValue(snapshot);const hook=renderHook(({world})=>useAutomaticRoad(bounds,true,world),{initialProps:{world:1}});await act(async()=>{await hook.result.current.ensureCurrent();});return hook;}
test('uses original snapshot and provider evidence without loading a new repository',async()=>{const h=await setup();mocks.read.mockResolvedValue(evidence);expect(await h.result.current.readTileEvidenceBatch([])).toEqual({status:'ready',evidence});expect(mocks.load).toHaveBeenCalledTimes(1);expect(h.result.current.snapshot).toBe(snapshot);});
test('late world evidence cannot become a construction result',async()=>{const h=await setup();let finish!:(x:typeof evidence)=>void;mocks.read.mockImplementation(()=>new Promise(r=>finish=r));const p=h.result.current.readTileEvidenceBatch([]);h.rerender({world:2});await act(async()=>finish(evidence));expect(await p).toEqual({status:'stale'});});
test('external cancellation propagates to evidence only and returns stale',async()=>{const h=await setup();const c=new AbortController();mocks.read.mockImplementation((_,signal:AbortSignal)=>new Promise((_,reject)=>signal.addEventListener('abort',()=>reject(Error('cancel')))));const p=h.result.current.readTileEvidenceBatch([],c.signal);c.abort();expect(await p).toEqual({status:'stale'});expect(mocks.load).toHaveBeenCalledTimes(1);});
test('provider 25 second deadline is reported unavailable without dropping observed roads',async()=>{const h=await setup();mocks.read.mockImplementation(()=>new Promise((_,reject)=>setTimeout(()=>reject(new DOMException('道路証拠取得が25秒で完了しませんでした。','TimeoutError')),25000)));const p=h.result.current.readTileEvidenceBatch([]);await act(()=>vi.advanceTimersByTimeAsync(25000));expect(await p).toMatchObject({status:'unavailable',reason:expect.stringContaining('25秒')});expect(h.result.current.snapshot).toBe(snapshot);});

test('reader identity remains stable across same-context renders',async()=>{const h=await setup();const reader=h.result.current.readTileEvidenceBatch;h.rerender({world:1});expect(h.result.current.readTileEvidenceBatch).toBe(reader);h.rerender({world:2});expect(h.result.current.readTileEvidenceBatch).not.toBe(reader);});

test('pinned acquisition has an independent reader and rejects results from a superseded world',async()=>{
 const h=await setup();let finish!:(value:typeof evidence)=>void;
 mocks.pinned.mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
 const request=h.result.current.readPinnedSeamTiles([]);
 expect(mocks.read).not.toHaveBeenCalled();
 h.rerender({world:2});
 await act(async()=>finish(evidence));
 expect(await request).toEqual({status:'stale'});
});

test('a timeout from a retired road observation is stale, not a retryable transport error',async()=>{
 const h=await setup();let reject!:(e:unknown)=>void;
 mocks.observe.mockImplementation(()=>new Promise((_,fail)=>{reject=fail;}));
 const request=h.result.current.observeRouteTiles([[14552,6451]],'fixed');
 const result=expect(request).rejects.toThrow('道路取得範囲が変わりました');
 h.rerender({world:2});
 await act(async()=>reject(new DOMException('deadline','TimeoutError')));
 await result;
});
