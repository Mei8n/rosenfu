import {readWaterEvidenceComposite,type WaterEvidenceComposite} from './waterEvidenceComposite.ts';
export type WaterEvidenceDelivery={directoryUrl:string;compositeSha256:string};
/** Download inspection evidence only. A successful request never certifies coverage. */
export async function loadWaterEvidenceComposite(descriptor:WaterEvidenceDelivery,options:{signal?:AbortSignal;fetchImpl?:typeof fetch}={}):Promise<WaterEvidenceComposite>{
 const base=new URL(descriptor.directoryUrl),pin=descriptor.compositeSha256;
 if(base.protocol!=='https:'||base.username||base.password||base.search||base.hash||!base.pathname.endsWith('/')||!/^[a-f0-9]{64}$/.test(pin))throw Error('Invalid water evidence delivery descriptor');
 const controller=new AbortController(),abort=()=>controller.abort(options.signal?.reason);
 if(options.signal?.aborted)abort();else options.signal?.addEventListener('abort',abort,{once:true});
 const timer=setTimeout(()=>controller.abort(new DOMException('Water evidence request timed out','TimeoutError')),30000);
 let childTotal=0;
 function check(){controller.signal.throwIfAborted();}
 async function read(path:string,limit:number){
  check();const url=new URL(path,base);if(url.origin!==base.origin||!url.pathname.startsWith(base.pathname))throw Error('Unsafe water evidence path');
  const response=await (options.fetchImpl??fetch)(url,{signal:controller.signal,credentials:'omit',redirect:'error',cache:'no-store'});
  if(controller.signal.aborted){await response.body?.cancel().catch(()=>{});check();}
  if(!response.ok||response.redirected||!response.body)throw Error('Water evidence request failed');
  const reader=response.body.getReader(),chunks:Uint8Array[]=[];let size=0;
  const cancelRead=()=>{void reader.cancel().catch(()=>{});};
  controller.signal.addEventListener('abort',cancelRead,{once:true});
  try{
   for(;;){check();const item=await reader.read();check();if(item.done)break;
    size+=item.value.byteLength;if(path!=='composite.json')childTotal+=item.value.byteLength;
    if(size>limit||childTotal>20*1024*1024)throw Error('Water evidence response exceeds limit');
    chunks.push(item.value);
   }
  }catch(error){await reader.cancel().catch(()=>{});throw error;}finally{controller.signal.removeEventListener('abort',cancelRead);reader.releaseLock();}
  const result=new Uint8Array(size);let offset=0;for(const chunk of chunks){result.set(chunk,offset);offset+=chunk.length;}return result;
 }
 try{
  const index=await read('composite.json',65536);
  const actual=[...new Uint8Array(await crypto.subtle.digest('SHA-256',index))].map(v=>v.toString(16).padStart(2,'0')).join('');
  check();if(actual!==pin)throw Error('Water evidence composite pin mismatch');
  const m=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(index));
  if(!m||m.format!=='rosenfu-water-composite'||m.version!==1||m.geographic_completeness!=='unknown'||!Array.isArray(m.coverage)||m.coverage.length||m.source_time_policy!=='retain-per-part-revisions'||!Array.isArray(m.parts)||m.parts.length<2||m.parts.length>16)throw Error('Invalid water evidence composite');
  const seen=new Set<string>();
  // Validate every path before making any child request, even for a pinned index.
  for(const p of m.parts){if(!p||typeof p.key!=='string'||!/^[a-z][a-z0-9_-]{0,31}$/.test(p.key)||seen.has(p.key)||p.manifest!==`${p.key}/manifest.json`||p.payload!==`${p.key}/evidence.json`||typeof p.manifest_sha256!=='string'||!/^[a-f0-9]{64}$/.test(p.manifest_sha256))throw Error('Invalid water evidence part');seen.add(p.key);}
  const files=new Map<string,Uint8Array>();
  for(const p of m.parts)for(const path of [p.manifest,p.payload])files.set(path,await read(path,5*1024*1024));
  const result=await readWaterEvidenceComposite(index,pin,files);check();return result;
 }finally{clearTimeout(timer);options.signal?.removeEventListener('abort',abort);controller.abort();}
}
