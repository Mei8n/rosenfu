import {it,expect,vi} from 'vitest';
import {loadWaterEvidenceComposite} from './waterEvidenceDelivery';
const encode=(v:unknown)=>new TextEncoder().encode(JSON.stringify(v));
async function hash(b:Uint8Array){return [...new Uint8Array(await crypto.subtle.digest('SHA-256',b as Uint8Array<ArrayBuffer>))].map(v=>v.toString(16).padStart(2,'0')).join('');}
async function fixture(){
 const files=new Map<string,Uint8Array>(),parts=[];
 for(const key of ['inland','marine']){
  const d={format:'rosenfu-water-evidence',version:1,revision:key,bounds:[134,34,134.1,34.1],geographic_completeness:'unknown',coverage:[],observations:[],source_issues:[]};
  const payload=encode(d),manifest=encode({format:'rosenfu-water-evidence-manifest',version:1,revision:key,bounds:d.bounds,geographic_completeness:'unknown',coverage:[],source:{url:'https://source.test',file_sha256:'a'.repeat(64),bytes:1,license:'ODbL-1.0',license_url:'https://license.test',attribution:'OSM',selection:'test'},payload:{file:'evidence.json',bytes:payload.length,sha256:await hash(payload),counts:{'water-area':0,'waterway-line':0,coastline:0}},limitations:['No completeness']});
  files.set(`${key}/manifest.json`,manifest);files.set(`${key}/evidence.json`,payload);parts.push({key,manifest:`${key}/manifest.json`,payload:`${key}/evidence.json`,manifest_sha256:await hash(manifest)});
 }
 const index={format:'rosenfu-water-composite',version:1,geographic_completeness:'unknown',coverage:[],source_time_policy:'retain-per-part-revisions',parts};
 const bytes=encode(index);files.set('composite.json',bytes);
 const descriptor={directoryUrl:'https://packs.test/release/cell/',compositeSha256:await hash(bytes)};
 const fetchImpl=vi.fn<typeof fetch>(async(url)=>{const path=new URL(String(url)).pathname.replace('/release/cell/','');const b=files.get(path);return b?new Response(new Uint8Array(b),{status:200}):new Response(null,{status:404});});
 return {files,index,descriptor,fetchImpl};
}
it('loads pinned mixed revisions only as incomplete and sends no credentials',async()=>{const f=await fixture();const r=await loadWaterEvidenceComposite(f.descriptor,{fetchImpl:f.fetchImpl});expect(r.status).toBe('incomplete');expect(r.coverage).toEqual([]);expect(r.hasMixedRevisionClaims).toBe(true);expect(f.fetchImpl).toHaveBeenCalledTimes(5);expect(f.fetchImpl.mock.calls[0][1]).toMatchObject({credentials:'omit',redirect:'error',cache:'no-store'});});
it('rejects changed root before fetching children',async()=>{const f=await fixture();f.files.set('composite.json',encode({}));await expect(loadWaterEvidenceComposite(f.descriptor,{fetchImpl:f.fetchImpl})).rejects.toThrow('pin mismatch');expect(f.fetchImpl).toHaveBeenCalledTimes(1);});
it('validates all pinned paths before child requests',async()=>{const f=await fixture();f.index.parts[1].payload='https://other.test/data';const b=encode(f.index);f.files.set('composite.json',b);f.descriptor.compositeSha256=await hash(b);await expect(loadWaterEvidenceComposite(f.descriptor,{fetchImpl:f.fetchImpl})).rejects.toThrow();expect(f.fetchImpl).toHaveBeenCalledTimes(1);});
it('rejects a missing part and permits an independent retry',async()=>{const f=await fixture(),b=f.files.get('marine/evidence.json')!;f.files.delete('marine/evidence.json');await expect(loadWaterEvidenceComposite(f.descriptor,{fetchImpl:f.fetchImpl})).rejects.toThrow();f.files.set('marine/evidence.json',b);expect((await loadWaterEvidenceComposite(f.descriptor,{fetchImpl:f.fetchImpl})).status).toBe('incomplete');});
it('does not start requests after caller cancellation',async()=>{const f=await fixture(),c=new AbortController();c.abort();await expect(loadWaterEvidenceComposite(f.descriptor,{fetchImpl:f.fetchImpl,signal:c.signal})).rejects.toThrow();expect(f.fetchImpl).not.toHaveBeenCalled();});
it('cancels an oversized stream without accepting an empty result',async()=>{const f=await fixture(),cancel=vi.fn();const fetchImpl=vi.fn<typeof fetch>(async()=>new Response(new ReadableStream({start(c){c.enqueue(new Uint8Array(65537));},cancel})));await expect(loadWaterEvidenceComposite(f.descriptor,{fetchImpl})).rejects.toThrow('limit');expect(cancel).toHaveBeenCalled();});
it('rejects insecure or credential-bearing delivery roots',async()=>{const f=await fixture();for(const url of ['http://packs.test/','https://u:p@packs.test/','https://packs.test/no-slash','https://packs.test/?a=1'])await expect(loadWaterEvidenceComposite({...f.descriptor,directoryUrl:url},{fetchImpl:f.fetchImpl})).rejects.toThrow();expect(f.fetchImpl).not.toHaveBeenCalled();});

it('cancels an active body read and never starts children',async()=>{
 const f=await fixture(),controller=new AbortController(),cancel=vi.fn();let started!:()=>void;
 const ready=new Promise<void>(resolve=>{started=resolve;});
 const fetchImpl=vi.fn<typeof fetch>(async()=>new Response(new ReadableStream({pull(){started();},cancel})));
 const pending=loadWaterEvidenceComposite(f.descriptor,{fetchImpl,signal:controller.signal});
 const rejected=expect(pending).rejects.toThrow();await ready;controller.abort();await rejected;
 expect(cancel).toHaveBeenCalled();expect(fetchImpl).toHaveBeenCalledTimes(1);
});
it('aborts a pending fetch at the operation deadline',async()=>{
 const f=await fixture();vi.useFakeTimers();
 try{
  const fetchImpl=vi.fn<typeof fetch>((_url,init)=>new Promise((_resolve,reject)=>{init!.signal!.addEventListener('abort',()=>reject(init!.signal!.reason),{once:true});}));
  const pending=loadWaterEvidenceComposite(f.descriptor,{fetchImpl});const rejected=expect(pending).rejects.toThrow('timed out');
  await vi.advanceTimersByTimeAsync(30000);await rejected;expect(fetchImpl).toHaveBeenCalledTimes(1);
 }finally{vi.useRealTimers();}
});
