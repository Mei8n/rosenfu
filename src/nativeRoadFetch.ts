import {invoke,isTauri} from '@tauri-apps/api/core';
const LIMIT=5*1024*1024;
/** Transport adapter only. Native chooses the compiled release; the webview never
 * supplies a URL, release pin or filesystem path. Existing readers verify hashes. */
export function createNativeRoadFetcher(origin:string):typeof fetch {
 return async (input:RequestInfo|URL,init?:RequestInit)=>{
  const url=new URL(typeof input==='string'?input:input instanceof URL?input.href:input.url);
  if(!isTauri()||url.origin!==origin||url.username||url.password||url.search||url.hash||!url.pathname.startsWith('/road-data/')||(init?.method&&init.method!=='GET'))throw Error('道路配信の要求先が不正です');
  const path=url.pathname.slice('/road-data/'.length);
  if(path!=='index.json'&&!/^packs\/[a-z0-9-]+\/-?\d+_-?\d+\/(manifest|-?\d+_-?\d+)\.json$/.test(path))throw Error('道路配信パスが不正です');
  if(init?.signal?.aborted)throw new DOMException('Aborted','AbortError');
  let abort:(()=>void)|undefined;
  try{
   const data=await Promise.race([invoke<unknown>('road_delivery_read',{path}),new Promise<never>((_,reject)=>{abort=()=>reject(new DOMException('Aborted','AbortError'));init?.signal?.addEventListener('abort',abort,{once:true});})]);
   if(init?.signal?.aborted)throw new DOMException('Aborted','AbortError');
   if(!Array.isArray(data)||data.length>LIMIT||data.some(n=>!Number.isInteger(n)||n<0||n>255))throw Error('道路配信の応答サイズまたは形式が不正です');
   return new Response(Uint8Array.from(data),{headers:{'content-type':'application/json','content-length':String(data.length)}});
  }finally{if(abort)init?.signal?.removeEventListener('abort',abort);}
 };
}
