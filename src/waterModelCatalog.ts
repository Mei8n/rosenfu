import {fetchWaterFile,fetchFineWaterMask,fetchWorldWaterMask,fineWaterSpecCovers,lookupConstructionWater,
 type FineWaterSpec,type FineWaterMask,type ConstructionWaterLookup,type FineWaterAvailability,type CoarseWaterAvailability} from './waterMaskModel.ts';
export interface WaterCatalogPin {url:string;bytes:number;sha256:string}
type Bounds=[number,number,number,number];
type Payload={file:string;bytes:number;sha256:string};
interface Catalog {
 format:'rosenfu-water-model-catalog';version:1;revision:string;scope:Bounds[];fineListing:'complete';
 fine:{spec:FineWaterSpec;payload:Payload}[];
 coarse:{state:'not_provided'}|{state:'provided';sourceId:string;payload:Payload};
}
export type CatalogWaterResult={catalogRevision:string|null;lookup:ConstructionWaterLookup;
 failure:null|'catalog_unavailable'|'outside_catalog_scope'|'fine_unavailable'|'coarse_unavailable'};
const text=(v:unknown)=>typeof v==='string'&&v.length>0&&v.length<=256;
function payload(p:Payload):void {
 if(!p||typeof p.file!=='string'||! /^[a-zA-Z0-9_-]+(?:\/[a-zA-Z0-9_-]+)*\.(?:bin|water)$/.test(p.file)||
  !Number.isSafeInteger(p.bytes)||p.bytes<1||p.bytes>16*1024*1024||typeof p.sha256!=='string'||! /^[a-f0-9]{64}$/.test(p.sha256))throw Error('Invalid water payload');
}
function parse(bytes:Uint8Array):Catalog {
 const c=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes)) as Catalog;
 if(!c||c.format!=='rosenfu-water-model-catalog'||c.version!==1||!text(c.revision)||c.fineListing!=='complete'||
  !Array.isArray(c.scope)||!c.scope.length||c.scope.length>1024||!Array.isArray(c.fine)||c.fine.length>1024)throw Error('Invalid water catalog');
 for(const b of c.scope)if(!Array.isArray(b)||b.length!==4||!b.every(Number.isFinite)||
  !(-180<=b[0]&&b[0]<b[2]&&b[2]<=180&&-85<=b[1]&&b[1]<b[3]&&b[3]<=85))throw Error('Invalid catalog scope');
 const ids=new Set<string>();
 for(const f of c.fine){if(!f||!f.spec)throw Error('Invalid fine entry');fineWaterSpecCovers(f.spec,0,0);payload(f.payload);if(ids.has(f.spec.id))throw Error('Duplicate fine entry');ids.add(f.spec.id);}
 if(!c.coarse||!['not_provided','provided'].includes(c.coarse.state))throw Error('Missing coarse availability');
 if(c.coarse.state==='provided'){if(!text(c.coarse.sourceId))throw Error('Invalid coarse source');payload(c.coarse.payload);}
 return c;
}
/** The pin must come from trusted application configuration, never from the same
 * untrusted response. Scope certifies the catalog's listing, not real-world water.
 * Only a verified complete listing may establish normal non-provision. */
export async function loadConstructionWaterAt(pin:WaterCatalogPin,lon:number,lat:number,
 options:{signal?:AbortSignal;fetcher?:typeof fetch}={}):Promise<CatalogWaterResult> {
 if(!Number.isFinite(lon)||!Number.isFinite(lat)||Math.abs(lon)>180||Math.abs(lat)>85)return {catalogRevision:null,lookup:{state:'unknown',reason:'invalid_coordinate'},failure:'outside_catalog_scope'};
 const fixed={...pin},externalSignal=options.signal;
 const base=new URL(fixed.url);
 if(base.protocol!=='https:'||base.username||base.password||base.search||base.hash||!Number.isSafeInteger(fixed.bytes)||fixed.bytes<1||fixed.bytes>1024*1024)throw Error('Invalid catalog pin');
 const controller=new AbortController(),abort=()=>controller.abort(externalSignal?.reason);
 if(externalSignal?.aborted)abort();else externalSignal?.addEventListener('abort',abort,{once:true});
 const timer=setTimeout(()=>controller.abort(new DOMException('Water model timeout','TimeoutError')),30000);
 const fetcher=options.fetcher??fetch;
 async function retry<T>(read:()=>Promise<T>):Promise<T>{
  for(let attempt=0;;attempt++){
   controller.signal.throwIfAborted();
   try{return await read();}catch(error){controller.signal.throwIfAborted();if(attempt===1)throw error;}
  }
 }
 const unavailable=(revision:string|null,failure:CatalogWaterResult['failure'],reason:'fine_unavailable'|'coarse_unavailable'):CatalogWaterResult=>({catalogRevision:revision,lookup:{state:'unknown',reason},failure});
 try{
  let c:Catalog;
  try{c=await retry(async()=>parse(await fetchWaterFile(fixed.url,fixed.bytes,fixed.sha256,controller.signal,fetcher)));}
  catch(error){controller.signal.throwIfAborted();return unavailable(null,'catalog_unavailable','fine_unavailable');}
  if(!c.scope.some(b=>b[0]<=lon&&lon<b[2]&&b[1]<=lat&&lat<b[3]))return unavailable(c.revision,'outside_catalog_scope','fine_unavailable');
  const required=c.fine.filter(f=>fineWaterSpecCovers(f.spec,lon,lat));
  if(required.length>8)return unavailable(c.revision,'fine_unavailable','fine_unavailable');
  const loaded=new Map<string,FineWaterMask>();
  try{for(const f of required)loaded.set(f.spec.id,await retry(()=>fetchFineWaterMask(f.spec,new URL(f.payload.file,base).href,f.payload.bytes,f.payload.sha256,controller.signal,fetcher)));}
  catch(error){controller.signal.throwIfAborted();return unavailable(c.revision,'fine_unavailable','fine_unavailable');}
  const fine:FineWaterAvailability=required.length?{state:'ready',catalog:required.map(f=>f.spec),loaded}:{state:'not_provided',catalogRevision:c.revision};
  // A required fine grid (including an incomplete observation) decides this point;
  // unrelated coarse failure must not hide or replace that result.
  if(required.length){
   try{return {catalogRevision:c.revision,lookup:lookupConstructionWater(lon,lat,fine,{state:'unavailable'}),failure:null};}
   catch{return unavailable(c.revision,'fine_unavailable','fine_unavailable');}
  }
  let coarse:CoarseWaterAvailability={state:'not_provided',catalogRevision:c.revision};
  if(c.coarse.state==='provided'){
   const entry=c.coarse;
   try{coarse={state:'ready',mask:await retry(()=>fetchWorldWaterMask(entry.sourceId,new URL(entry.payload.file,base).href,entry.payload.bytes,entry.payload.sha256,controller.signal,fetcher))};}
   catch(error){controller.signal.throwIfAborted();return unavailable(c.revision,'coarse_unavailable','coarse_unavailable');}
  }
  return {catalogRevision:c.revision,lookup:lookupConstructionWater(lon,lat,fine,coarse),failure:null};
 }finally{clearTimeout(timer);externalSignal?.removeEventListener('abort',abort);controller.abort();}
}
