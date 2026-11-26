// @vitest-environment jsdom
import {act,cleanup,renderHook} from '@testing-library/react';
import {afterEach,expect,it,vi} from 'vitest';
import {PbfWriter} from 'pbf';
import {useAutomaticRoad} from './useAutomaticRoad';

afterEach(()=>{cleanup();vi.useRealTimers();vi.unstubAllGlobals();});

it('starts the new viewport tile without waiting for a stalled offscreen tile to time out',async()=>{
 vi.useFakeTimers();
 const p=new PbfWriter();
 p.writeMessage(3,(_,layer)=>{
  layer.writeStringField(1,'transportation');layer.writeVarintField(15,2);layer.writeVarintField(5,4096);
  layer.writeStringField(3,'class');layer.writeMessage(4,(_,v)=>v.writeStringField(1,'service'),null);
  layer.writeMessage(2,(_,f)=>{
   f.writePackedVarint(2,[0,0]);f.writeVarintField(3,2);f.writePackedVarint(4,[9,200,200,10,200,0]);
  },null);
 },null);
 const raw=p.finish(),asked:string[]=[];
 let offscreenSignal:AbortSignal|undefined;
 vi.stubGlobal('fetch',vi.fn(async(url:string,options?:RequestInit)=>{
  asked.push(url);
  if(url.endsWith('/planet'))return new Response(JSON.stringify({maxzoom:14,tiles:['https://tiles.openfreemap.org/planet/rev/{z}/{x}/{y}.pbf']}));
  if(url.endsWith('/14552/6451.pbf')){
   offscreenSignal=options!.signal!;
   return new Promise<Response>((_,reject)=>offscreenSignal!.addEventListener('abort',()=>reject(new DOMException('aborted','AbortError')),{once:true}));
  }
  return new Response(new Uint8Array(raw));
 }));
 const bounds={west:139.75,east:139.751,south:35.681,north:35.682};
 const {result,rerender}=renderHook(({west,east})=>useAutomaticRoad({...bounds,west,east},true,1),{initialProps:bounds});
 await act(()=>vi.advanceTimersByTimeAsync(300));
 expect(asked.map(url=>new URL(url).pathname)).toEqual(['/planet','/planet/rev/14/14552/6451.pbf']);
 rerender({...bounds,west:139.771,east:139.772});
 await act(()=>vi.advanceTimersByTimeAsync(300));
 expect(offscreenSignal?.aborted).toBe(true);
 expect(asked.map(url=>new URL(url).pathname)).toEqual(['/planet','/planet/rev/14/14552/6451.pbf','/planet/rev/14/14553/6451.pbf']);
 await vi.waitFor(()=>expect(result.current.status).toBe('ready'));
 expect(result.current.snapshot?.tileSnapshots?.[0].bundle.source.source).toContain('/14553/6451.pbf');
 expect(result.current.snapshot?.coverageVerified).toBe(false);
});
