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

test('hung native read respects deadline and retries without interpreting absence',async()=>{
 vi.useFakeTimers();try{vi.resetModules();const {useWaterModelAcquisition}=await import('./useWaterModelAcquisition');let calls=0;vi.mocked(invoke).mockImplementation(()=>{calls++;return new Promise(()=>{});});const {result,unmount}=renderHook(()=>useWaterModelAcquisition(true,1));await act(async()=>{});expect(calls).toBe(1);await act(async()=>vi.advanceTimersByTimeAsync(30000));expect(result.current).toBe(0);await act(async()=>vi.advanceTimersByTimeAsync(30000));expect(calls).toBe(2);await act(async()=>vi.advanceTimersByTimeAsync(90000));expect(calls).toBe(3);expect(result.current).toBe(0);unmount();}finally{vi.useRealTimers();}
});
test('late cancelled read never starts fetch or install and late rejection is handled',async()=>{
 vi.resetModules();const {acquireWaterMasks}=await import('./useWaterModelAcquisition');let finish;const call=vi.fn(()=>new Promise(r=>finish=r)),fetcher=vi.fn();const controller=new AbortController();const result=acquireWaterMasks(controller.signal,vi.fn(),call,fetcher);await Promise.resolve();controller.abort(Error('cancel'));await expect(result).rejects.toThrow('cancel');finish([entry('fuji').pin]);await Promise.resolve();await Promise.resolve();expect(fetcher).not.toHaveBeenCalled();expect(call).toHaveBeenCalledTimes(1);
 const controller2=new AbortController();let fail;const result2=acquireWaterMasks(controller2.signal,vi.fn(),vi.fn(()=>new Promise((_,r)=>fail=r)),fetcher);await Promise.resolve();controller2.abort(Error('cancel2'));await expect(result2).rejects.toThrow('cancel2');fail(Error('late native rejection'));await Promise.resolve();
});
test('outstanding native calls remain bounded across aborted world acquisitions',async()=>{
 vi.resetModules();const {acquireWaterMasks}=await import('./useWaterModelAcquisition');const call=vi.fn(()=>new Promise(()=>{}));for(let i=0;i<12;i++){const c=new AbortController(),result=acquireWaterMasks(c.signal,vi.fn(),call,vi.fn());await Promise.resolve();c.abort();await expect(result).rejects.toBeDefined();}expect(call).toHaveBeenCalledTimes(8);
});
test('late timed-out install notifies only a current hook and requires a fresh native query',async()=>{
 vi.resetModules();const {acquireWaterMasks,useWaterModelAcquisition}=await import('./useWaterModelAcquisition');const {data,pin}=entry('fuji');let finishInstall;const oldInstalled=vi.fn(),oldCall=vi.fn(command=>command==='water_mask_requests'?Promise.resolve([pin]):new Promise(resolve=>finishInstall=resolve));const controller=new AbortController();const old=acquireWaterMasks(controller.signal,oldInstalled,oldCall,async()=>new Response(data));await waitFor(()=>expect(finishInstall).toBeTruthy());controller.abort(Error('deadline'));await expect(old).rejects.toThrow('deadline');
 vi.mocked(invoke).mockResolvedValue([]);const {result,unmount}=renderHook(()=>useWaterModelAcquisition(true,'current-world'));await waitFor(()=>expect(result.current).toBe(1));const before=vi.mocked(invoke).mock.calls.length;await act(async()=>finishInstall());await waitFor(()=>expect(vi.mocked(invoke).mock.calls.length).toBeGreaterThan(before));await waitFor(()=>expect(result.current).toBe(2));expect(oldInstalled).not.toHaveBeenCalled();expect(vi.mocked(invoke).mock.calls.every(([c])=>c==='water_mask_requests')).toBe(true);unmount();
});
