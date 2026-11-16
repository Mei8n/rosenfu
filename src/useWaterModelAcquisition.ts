import {useEffect,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {fetchWaterFile} from './waterMaskModel';
type Request={id:string;native:true}|{id:string;native?:false;url:string;bytes:number;sha256:string};
const cached=new Map<string,Uint8Array>();
const ROOT='https://raw.githubusercontent.com/Mei8n/rosenfu/108195ed2fe57b5ab8ea4f787a560a5fa30362b4/data/water-mask-trial-20261114/';
function requests(value:unknown):Request[]{
 if(!Array.isArray(value)||value.length>45)throw Error('水域配信一覧を確認できません。');
 const ids=new Set<string>();
 return value.map(v=>{
  if(v&&typeof v==='object'&&v.native===true){if(typeof v.id!=='string'||!/^[a-z0-9-]{1,32}$/.test(v.id)||ids.has(v.id))throw Error('水域配信一覧が不正です。');ids.add(v.id);return {id:v.id,native:true as const};}
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
 let native:Promise<T>;
 // Preserve the established dispatch order; wrap synchronous transport errors too.
 try{native=Promise.resolve(call<T>(command,args));}catch(error){native=Promise.reject(error);}
 return new Promise<T>((resolve,reject)=>{
  const abort=()=>reject(signal.reason??new Error('水域取得を中止しました。'));
  signal.addEventListener('abort',abort,{once:true});
  native.then(value=>{
   pendingIpc--;signal.removeEventListener('abort',abort);
   if(signal.aborted){
    if(command==='install_water_mask'||command==='acquire_water_zone')for(const notify of [...lateInstallListeners])notify();
    return;
   }
   resolve(value);
  },error=>{pendingIpc--;signal.removeEventListener('abort',abort);if(!signal.aborted)reject(error);});
  if(signal.aborted)abort();
 });
}
/** Downloads are independent: failure of one area never prevents installing another. */
export async function acquireWaterMasks(signal:AbortSignal,installed:()=>void,call:typeof invoke=invoke,fetcher:typeof fetch=fetch,bounds?:[number,number,number,number],queue?:{offset:number;listed:(count:number)=>void}):Promise<boolean>{
 const requested=requests(await waterCall(call,'water_mask_requests',bounds?{bounds}:undefined,signal));
 queue?.listed(requested.length);const offset=requested.length?(queue?.offset??0)%requested.length:0;const list=[...requested.slice(offset),...requested.slice(0,offset)];
 async function install(item:Request){
  if(item.native){await waterCall(call,'acquire_water_zone',{id:item.id},signal);if(!signal.aborted)installed();return;}

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
 }
 // At most two independent area transfers; one failing area does not serialize all others.
 let complete=true,index=0;
 const worker=async()=>{while(index<list.length&&!signal.aborted){const item=list[index++];try{await install(item);}catch{complete=false;}}};
 await Promise.all(Array.from({length:Math.min(2,list.length)},worker));
 if(signal.aborted)throw signal.reason;
 return complete;
}
/** No new UI: native preview/build owns refusal reasons. Fixed bytes remain in the session cache. */
export function useWaterModelAcquisition(enabled:boolean,world:unknown,bounds?:[number,number,number,number]){
 const boundsKey=JSON.stringify(bounds??null);
 const [revision,setRevision]=useState(0);
 const [cacheChange,setCacheChange]=useState(0);
 useEffect(()=>{if(!enabled)return;const notify=()=>setCacheChange(v=>v+1);lateInstallListeners.add(notify);return()=>{lateInstallListeners.delete(notify);};},[enabled]);
 useEffect(()=>{
  if(!enabled)return;let disposed=false;let timer:ReturnType<typeof setTimeout>|undefined;let active:AbortController|undefined;
  async function run(round:number,noProgress:number){
   const controller=new AbortController();active=controller;const deadline=setTimeout(()=>controller.abort(new Error('水域取得がタイムアウトしました。')),30000);
   let complete=false,progress=0,listed=0;
   try{complete=await acquireWaterMasks(controller.signal,()=>{progress++;if(!disposed)setRevision(v=>v+1);},invoke,fetch,JSON.parse(boundsKey)??undefined,{offset:round*2,listed:count=>{listed=count;}});}catch{/* Native retains unavailable rather than treating errors as absence. */}
   finally{clearTimeout(deadline);}
   // A previous effect may have completed its native install after cancellation.
   // A fresh empty missing-list is also evidence that preview must be refreshed.
   if(!disposed&&complete)setRevision(v=>v+1);
   const stalled=progress?0:noProgress+1;
   // Every queued pair gets an opportunity; successful partial rounds do not spend the failure budget.
   if(!disposed&&!complete&&round<47&&stalled<Math.max(3,Math.ceil(listed/2)+1))timer=setTimeout(()=>void run(round+1,stalled),30000);
  }
  void run(0,0);
  return()=>{disposed=true;clearTimeout(timer);active?.abort();};
 },[enabled,world,cacheChange,boundsKey]);
 return revision;
}
