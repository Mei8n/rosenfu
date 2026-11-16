// @vitest-environment jsdom
import {act,cleanup,renderHook,waitFor} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {webcrypto} from 'node:crypto';
import {beforeAll,afterEach} from 'vitest';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
beforeAll(()=>vi.stubGlobal('crypto',webcrypto));
afterEach(()=>{cleanup();vi.mocked(invoke).mockReset();});
import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {expect,test,vi} from 'vitest';
const root='https://raw.githubusercontent.com/Mei8n/rosenfu/108195ed2fe57b5ab8ea4f787a560a5fa30362b4/data/water-mask-trial-20261114/';
function entry(id){const data=readFileSync(`data/water-mask-trial-20261114/${id}/water.bin`);return {data,pin:{id,url:root+id+'/water.bin',bytes:data.length,sha256:createHash('sha256').update(data).digest('hex')}};}
test('verified real fine bytes reach native unchanged; fresh session reuses verified cache',async()=>{
 vi.resetModules();const {acquireWaterMasks}=await import('./useWaterModelAcquisition');const {data,pin}=entry('kanmon');const installs=[];
 const call=vi.fn(async(command,args)=>{if(command==='water_mask_requests')return [pin];installs.push(args);});
 const fetcher=vi.fn(async()=>new Response(data,{headers:{'Content-Length':String(data.length)}}));const done=vi.fn();
 expect(await acquireWaterMasks(new AbortController().signal,done,call,fetcher)).toBe(true);
 expect(installs).toEqual([{id:'kanmon',bytesBase64:data.toString('base64')}]);expect(done).toHaveBeenCalledTimes(1);
 await acquireWaterMasks(new AbortController().signal,done,call,fetcher);expect(fetcher).toHaveBeenCalledTimes(1);expect(installs).toHaveLength(2);
});
test('404 retries without absent success; a different valid area still installs',async()=>{
 vi.resetModules();const {acquireWaterMasks}=await import('./useWaterModelAcquisition');const a=entry('kanmon'),b=entry('takamatsu');const installs=[];
 const call=vi.fn(async(command,args)=>command==='water_mask_requests'?[a.pin,b.pin]:installs.push(args));
 const fetcher=vi.fn(async(url)=>String(url)===a.pin.url?new Response(null,{status:404}):new Response(b.data,{headers:{'Content-Length':String(b.data.length)}}));
 expect(await acquireWaterMasks(new AbortController().signal,()=>{},call,fetcher)).toBe(false);
 expect(installs).toEqual([{id:'takamatsu',bytesBase64:b.data.toString('base64')}]);expect(fetcher).toHaveBeenCalledTimes(3);
});
test('corruption and cancellation never install an empty mask',async()=>{
 vi.resetModules();const {acquireWaterMasks}=await import('./useWaterModelAcquisition');const {data,pin}=entry('fuji');const call=vi.fn(async(command)=>command==='water_mask_requests'?[pin]:undefined);
 const corrupt=Buffer.from(data);corrupt[0]^=1;const fetcher=vi.fn(async()=>new Response(corrupt,{headers:{'Content-Length':String(data.length)}}));
 expect(await acquireWaterMasks(new AbortController().signal,()=>{},call,fetcher)).toBe(false);expect(fetcher).toHaveBeenCalledTimes(2);expect(call).toHaveBeenCalledTimes(1);
 const controller=new AbortController();controller.abort();await expect(acquireWaterMasks(controller.signal,()=>{},call,fetcher)).rejects.toBeDefined();expect(vi.mocked(call).mock.calls.every(c=>c[0]==='water_mask_requests')).toBe(true);
});

test('a late old native install plus a new empty missing-list refreshes a stale refusal',async()=>{
 vi.resetModules();const {useWaterModelAcquisition}=await import('./useWaterModelAcquisition');const {useConstructionPreview}=await import('./useConstructionPreview');
 const {data,pin}=entry('fuji');let installed=false,lists=0,finishInstall,finishList;
 const lateInstall=new Promise(resolve=>finishInstall=resolve),newList=new Promise(resolve=>finishList=resolve);
 vi.stubGlobal('fetch',vi.fn(async()=>new Response(data,{headers:{'Content-Length':String(data.length)}})));
 vi.mocked(invoke).mockImplementation(command=>{
  if(command==='water_mask_requests')return ++lists===1?Promise.resolve([pin]):newList;
  if(command==='install_water_mask')return lateInstall;
  if(command==='preview_construction_plan')return installed?Promise.resolve({ready:true}):Promise.reject('細密未取得');
  throw Error('unexpected command');
 });
 const request={alignments:[{}],stations:[]};const {result,rerender}=renderHook(({world})=>{const revision=useWaterModelAcquisition(true,world);return useConstructionPreview(world,request,revision);},{initialProps:{world:{id:1}}});
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('install_water_mask',expect.anything()));
 rerender({world:{id:2}});await waitFor(()=>expect(result.current?.error).toBe('細密未取得'));
 await act(async()=>{installed=true;finishInstall();finishList([]);});
 await waitFor(()=>expect(result.current?.value).toEqual({ready:true}));
});
