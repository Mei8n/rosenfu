import {useEffect,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {fetchWaterFile} from './waterMaskModel';
type Request={id:string;url:string;bytes:number;sha256:string};
const cached=new Map<string,Uint8Array>();
const ROOT='https://raw.githubusercontent.com/Mei8n/rosenfu/108195ed2fe57b5ab8ea4f787a560a5fa30362b4/data/water-mask-trial-20261114/';
function requests(value:unknown):Request[]{
 if(!Array.isArray(value)||value.length>3)throw Error('水域配信一覧を確認できません。');
 const ids=new Set<string>();
 return value.map(v=>{
  if(!v||typeof v!=='object'||!['fuji','kanmon','takamatsu'].includes(v.id)||ids.has(v.id)||v.url!==`${ROOT}${v.id}/water.bin`||!Number.isInteger(v.bytes)||v.bytes<1||v.bytes>4096||typeof v.sha256!=='string'||!/^[0-9a-f]{64}$/.test(v.sha256))throw Error('水域配信一覧が不正です。');
  ids.add(v.id);return {...v};
 });
}
// Native IPC cannot be cancelled. Bound outstanding calls across world changes,
// while allowing each caller to stop waiting at its own deadline.
const MAX_PENDING_IPC=8;
let pendingIpc=0;
const lateInstallListeners=new Set<()=>void>();
function waterCall<T>(call:typeof invoke,command:string,args:Parameters<typeof invoke>[1],signal:AbortSignal):Promise<T>{
 signal.throwIfAborted();
 if(pendingIpc>=MAX_PENDING_IPC)return Promise.reject(Error('水域IPCの完了待ちが上限です。'));
 pendingIpc++;
 const native=Promise.resolve().then(()=>{signal.throwIfAborted();return call<T>(command,args);});
 return new Promise<T>((resolve,reject)=>{
  const abort=()=>reject(signal.reason??new Error('水域取得を中止しました。'));
  signal.addEventListener('abort',abort,{once:true});
  native.then(value=>{
   pendingIpc--;signal.removeEventListener('abort',abort);
   if(signal.aborted){
    if(command==='install_water_mask')for(const notify of [...lateInstallListeners])notify();
    return;
   }
   resolve(value);
  },error=>{pendingIpc--;signal.removeEventListener('abort',abort);if(!signal.aborted)reject(error);});
  if(signal.aborted)abort();
 });
}
/** Downloads are independent: failure of one area never prevents installing another. */
export async function acquireWaterMasks(signal:AbortSignal,installed:()=>void,call:typeof invoke=invoke,fetcher:typeof fetch=fetch):Promise<boolean>{
 const list=requests(await waterCall(call,'water_mask_requests',undefined,signal));
 const outcomes=await Promise.allSettled(list.map(async item=>{
  let bytes:Uint8Array|undefined=cached.get(item.sha256);
  for(let attempt=0;!bytes&&attempt<2;attempt++){
   try{bytes=await fetchWaterFile(item.url,item.bytes,item.sha256,signal,fetcher);break;}
   catch(error){if(signal.aborted||attempt===1)throw error;}
  }
  if(signal.aborted)throw signal.reason;
  cached.set(item.sha256,bytes!);
  let binary='';for(const b of bytes!)binary+=String.fromCharCode(b);
  await waterCall(call,'install_water_mask',{id:item.id,bytesBase64:btoa(binary)},signal);
  if(!signal.aborted)installed();
 }));
 if(signal.aborted)throw signal.reason;
 return outcomes.every(x=>x.status==='fulfilled');
}
/** No new UI: native preview/build owns refusal reasons. Fixed bytes remain in the session cache. */
export function useWaterModelAcquisition(enabled:boolean,world:unknown){
 const [revision,setRevision]=useState(0);
 const [cacheChange,setCacheChange]=useState(0);
 useEffect(()=>{if(!enabled)return;const notify=()=>setCacheChange(v=>v+1);lateInstallListeners.add(notify);return()=>{lateInstallListeners.delete(notify);};},[enabled]);
 useEffect(()=>{
  if(!enabled)return;let disposed=false;let timer:ReturnType<typeof setTimeout>|undefined;let active:AbortController|undefined;
  async function run(round:number){
   const controller=new AbortController();active=controller;const deadline=setTimeout(()=>controller.abort(new Error('水域取得がタイムアウトしました。')),30000);
   let complete=false;
   try{complete=await acquireWaterMasks(controller.signal,()=>{if(!disposed)setRevision(v=>v+1);});}catch{/* Native retains unavailable rather than treating errors as absence. */}
   finally{clearTimeout(deadline);}
   // A previous effect may have completed its native install after cancellation.
   // A fresh empty missing-list is also evidence that preview must be refreshed.
   if(!disposed&&complete)setRevision(v=>v+1);
   if(!disposed&&!complete&&round<2)timer=setTimeout(()=>void run(round+1),30000);
  }
  void run(0);
  return()=>{disposed=true;clearTimeout(timer);active?.abort();};
 },[enabled,world,cacheChange]);
 return revision;
}
