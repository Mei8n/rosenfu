import {describe,it,expect,vi} from 'vitest';
import {loadConstructionWaterAt} from './waterModelCatalog';
const root='https://example.org/release/',encode=(v:unknown)=>new TextEncoder().encode(JSON.stringify(v));
const sha=async(b:Uint8Array<ArrayBuffer>)=>Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',b)),v=>v.toString(16).padStart(2,'0')).join('');
const catalog=()=>({format:'rosenfu-water-model-catalog',version:1,revision:'fixed-v1',scope:[[-180,-85,180,85]],fineListing:'complete',fine:[] as unknown[],coarse:{state:'not_provided'} as Record<string,unknown>});
async function fixture(c:unknown,files=new Map<string,Uint8Array>()){
 const bytes=encode(c),pin={url:root+'catalog.json',bytes:bytes.length,sha256:await sha(bytes)};
 files.set(pin.url,bytes);
 const fetcher=vi.fn<typeof fetch>(async input=>{const b=files.get(String(input));return b?new Response(new Uint8Array(b),{status:200}):new Response(null,{status:404});});
 return {pin,fetcher};
}
function spec(){return {id:'fine-v1',revision:'source-v1',coverage:'observations_only',originLongitude:139,originLatitude:35,x0:0,y0:0,cellM:25,width:1,height:1};}
function world(){const b=new Uint8Array(16+180*3);b.set([84,87,87,49]);const v=new DataView(b.buffer);v.setUint32(4,360,true);v.setUint32(8,180,true);v.setUint32(12,1,true);for(let r=0;r<180;r++)b.set([2,0,1],16+r*3);return b;}
describe('trusted water model catalog acquisition',()=>{
 it('uses explicit non-provision only inside verified complete catalog scope',async()=>{
  const c=catalog();c.scope=[[138,34,140,36]];const f=await fixture(c);
  const result=await loadConstructionWaterAt(f.pin,139,35,{fetcher:f.fetcher});
  expect(result).toEqual({catalogRevision:'fixed-v1',failure:null,lookup:{state:'model_land',basis:'not_provided',sourceIds:[]}});expect(f.fetcher).toHaveBeenCalledTimes(1);
  expect((await loadConstructionWaterAt(f.pin,140,35,{fetcher:f.fetcher})).failure).toBe('outside_catalog_scope');
 });
 it('retries transient catalog failure but never turns persistent 404 into absence',async()=>{
  const f=await fixture(catalog());f.fetcher.mockResolvedValueOnce(new Response(null,{status:503}));
  expect((await loadConstructionWaterAt(f.pin,139,35,{fetcher:f.fetcher})).lookup.state).toBe('model_land');expect(f.fetcher).toHaveBeenCalledTimes(2);
  const missing=vi.fn<typeof fetch>(async()=>new Response(null,{status:404}));
  expect((await loadConstructionWaterAt(f.pin,139,35,{fetcher:missing})).failure).toBe('catalog_unavailable');expect(missing).toHaveBeenCalledTimes(2);
 });
 it('rejects partial listings, unsafe paths and wrong hashes before trusting absence',async()=>{
  for(const c of [{...catalog(),fineListing:'partial'},{...catalog(),coarse:undefined},{...catalog(),fine:[{spec:spec(),payload:{file:'../water.bin',bytes:8,sha256:'a'.repeat(64)}}]}]){
   const f=await fixture(c);expect((await loadConstructionWaterAt(f.pin,139,35,{fetcher:f.fetcher})).failure).toBe('catalog_unavailable');expect(f.fetcher).toHaveBeenCalledTimes(2);
  }
  const f=await fixture(catalog());expect((await loadConstructionWaterAt({...f.pin,sha256:'0'.repeat(64)},139,35,{fetcher:f.fetcher})).failure).toBe('catalog_unavailable');
 });
 it('keeps a listed fine 404 unavailable after retry',async()=>{
  const c=catalog();c.fine=[{spec:spec(),payload:{file:'water.bin',bytes:8,sha256:'a'.repeat(64)}}];const f=await fixture(c);
  expect((await loadConstructionWaterAt(f.pin,139,35,{fetcher:f.fetcher})).failure).toBe('fine_unavailable');expect(f.fetcher).toHaveBeenCalledTimes(3);
 });
 it('never hides an observation gap with coarse non-provision or a failing coarse file',async()=>{
  const empty=new Uint8Array(8),c=catalog();c.fine=[{spec:spec(),payload:{file:'water.bin',bytes:8,sha256:await sha(empty)}}];
  c.coarse={state:'provided',sourceId:'coarse',payload:{file:'world.water',bytes:20,sha256:'a'.repeat(64)}};
  const f=await fixture(c,new Map([[root+'water.bin',empty]]));
  expect((await loadConstructionWaterAt(f.pin,139,35,{fetcher:f.fetcher})).lookup).toEqual({state:'unknown',reason:'fine_incomplete'});expect(f.fetcher).toHaveBeenCalledTimes(2);
 });
 it('uses verified coarse water and model land, but rejects hashed corrupt coarse bytes',async()=>{
  const bytes=world(),c=catalog();c.coarse={state:'provided',sourceId:'world-v1',payload:{file:'world.water',bytes:bytes.length,sha256:await sha(bytes)}};
  const f=await fixture(c,new Map([[root+'world.water',bytes]]));
  expect((await loadConstructionWaterAt(f.pin,-179.5,35,{fetcher:f.fetcher})).lookup).toMatchObject({state:'model_water',basis:'coarse'});
  expect((await loadConstructionWaterAt(f.pin,139,35,{fetcher:f.fetcher})).lookup).toMatchObject({state:'model_land',basis:'coarse'});
  bytes[16]=1;c.coarse={...c.coarse,payload:{file:'world.water',bytes:bytes.length,sha256:await sha(bytes)}};
  const broken=await fixture(c,new Map([[root+'world.water',bytes]]));expect((await loadConstructionWaterAt(broken.pin,139,35,{fetcher:broken.fetcher})).failure).toBe('coarse_unavailable');expect(broken.fetcher).toHaveBeenCalledTimes(3);
 });
 it('honors caller cancellation before any acquisition',async()=>{
  const f=await fixture(catalog()),abort=new AbortController();abort.abort();
  await expect(loadConstructionWaterAt(f.pin,139,35,{fetcher:f.fetcher,signal:abort.signal})).rejects.toThrow();expect(f.fetcher).not.toHaveBeenCalled();
 });
});

it('snapshots a catalog pin before retrying after caller mutation',async()=>{
 const f=await fixture(catalog()),original=f.pin.url;
 f.fetcher.mockImplementationOnce(async()=>{f.pin.url='https://other.example/catalog.json';f.pin.bytes=1;f.pin.sha256='0'.repeat(64);return new Response(null,{status:503});});
 expect((await loadConstructionWaterAt(f.pin,139,35,{fetcher:f.fetcher})).lookup.state).toBe('model_land');
 expect(f.fetcher.mock.calls.map(args=>args[0])).toEqual([original,original]);
});
it('bounds overlapping fine requests before downloading their bodies',async()=>{
 const c=catalog();c.fine=Array.from({length:9},(_,i)=>({spec:{...spec(),id:'fine-'+i},payload:{file:'water.bin',bytes:8,sha256:'a'.repeat(64)}}));
 const f=await fixture(c);expect((await loadConstructionWaterAt(f.pin,139,35,{fetcher:f.fetcher})).failure).toBe('fine_unavailable');expect(f.fetcher).toHaveBeenCalledTimes(1);
});
it('cancels a blocked catalog body at the overall deadline',async()=>{
 const f=await fixture(catalog()),cancel=vi.fn();
 f.fetcher.mockResolvedValue(new Response(new ReadableStream<Uint8Array>({cancel}),{status:200}));
 vi.useFakeTimers();
 try{
  const pending=loadConstructionWaterAt(f.pin,139,35,{fetcher:f.fetcher});const check=expect(pending).rejects.toThrow();
  await vi.advanceTimersByTimeAsync(30000);await check;expect(cancel).toHaveBeenCalled();expect(f.fetcher).toHaveBeenCalledTimes(1);
 }finally{vi.useRealTimers();}
});
