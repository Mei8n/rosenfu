import {describe,it,expect} from 'vitest';
import {readWaterEvidencePack} from './waterEvidencePack';
const encode=(o:unknown)=>new TextEncoder().encode(JSON.stringify(o));
async function hash(b:Uint8Array){return [...new Uint8Array(await crypto.subtle.digest('SHA-256',b as Uint8Array<ArrayBuffer>))].map(x=>x.toString(16).padStart(2,'0')).join('');}
const ring=[[134.01,34.01],[134.09,34.01],[134.09,34.09],[134.01,34.09],[134.01,34.01]];
const hole=[[134.03,34.03],[134.03,34.04],[134.04,34.04],[134.04,34.03],[134.03,34.03]];
const area={id:'relation/9007199254740993',kind:'water-area',geometry:{type:'Polygon',coordinates:[ring,hole]},tags:{natural:'water'}};
const line=(kind:string)=>({id:'way/2',kind,geometry:{type:'LineString',coordinates:[[134.02,34.02],[134.08,34.08]]},tags:{waterway:'stream'}});
async function fixture(observations:unknown[]=[area],edit?:(m:Record<string,any>,d:Record<string,any>)=>void){
 const d:Record<string,any>={format:'rosenfu-water-evidence',version:1,revision:'test-snapshot',bounds:[134,34,134.1,34.1],geographic_completeness:'unknown',coverage:[],observations,source_issues:[]};
 const counts=Object.fromEntries(['water-area','waterway-line','coastline'].map(k=>[k,observations.filter(o=>(o as {kind:string}).kind===k).length]));
 const m:Record<string,any>={format:'rosenfu-water-evidence-manifest',version:1,revision:d.revision,bounds:d.bounds,geographic_completeness:'unknown',coverage:[],source:{url:'https://source.test/water',file_sha256:'a'.repeat(64),bytes:100,license:'ODbL-1.0',license_url:'https://source.test/license',attribution:'test attribution',selection:'water observations'},payload:{file:'evidence.json',counts},limitations:['No absence certification']};
 edit?.(m,d);const bytes=encode(d);m.payload.bytes=bytes.length;m.payload.sha256=await hash(bytes);const manifest=encode(m);return {manifest,bytes,pin:await hash(manifest)};
}
const read=async(f:Awaited<ReturnType<typeof fixture>>)=>readWaterEvidencePack(f.manifest,f.pin,f.bytes);
describe('water evidence is not dry-land certification',()=>{
 it('retains holes, source identity and positive observations without granting coverage',async()=>{
  const r=await read(await fixture());expect(r.status).toBe('incomplete');expect(r.coverage).toEqual([]);expect(r.water[0].rings).toHaveLength(2);expect(r.observations[0].id).toBe(area.id);expect(r.source.attribution).toBe('test attribution');
 });
 it('empty observations remain unknown',async()=>{const r=await read(await fixture([]));expect(r.water).toEqual([]);expect(r.coverage).toEqual([]);expect(r.status).toBe('incomplete');});
 it('never invents surface widths for waterways or sea polygons from coastlines',async()=>{const r=await read(await fixture([line('waterway-line'),line('coastline')]));expect(r.water).toEqual([]);expect(r.observations).toHaveLength(2);expect(r.coverage).toEqual([]);});
 it.each(['manifest','payload'])('rejects %s tampering',async(which)=>{const f=await fixture();if(which==='manifest')f.manifest[0]=32;else f.bytes[0]=32;await expect(read(f)).rejects.toThrow();});
 it('rejects wrong manifest identity',async()=>{const f=await fixture();f.pin='f'.repeat(64);await expect(read(f)).rejects.toThrow();});
 it.each(['manifest','payload'])('rejects self-declared complete %s',async(which)=>{const f=await fixture([], (m,d)=>{(which==='manifest'?m:d).geographic_completeness='complete';});await expect(read(f)).rejects.toThrow();});
 it('rejects nonempty coverage even in a correctly hashed pack',async()=>{await expect(read(await fixture([],(_m,d)=>{d.coverage=[[134,34,134.1,34.1]];}))).rejects.toThrow();});
 it('rejects duplicate evidence identities',async()=>{await expect(read(await fixture([area,area]))).rejects.toThrow();});
 it('rejects truncated rings and unsupported geometry',async()=>{
  for(const geometry of [{type:'Polygon',coordinates:[ring.slice(0,-1)]},{type:'Point',coordinates:[134,34]},{type:'Polygon',coordinates:[]}])await expect(read(await fixture([{...area,geometry}]))).rejects.toThrow();
 });
 it('rejects mismatched revision, bounds and counts',async()=>{
  for(const edit of [(m:Record<string,any>)=>{m.revision='other';},(m:Record<string,any>)=>{m.bounds=[134,34,134.05,34.05];},(m:Record<string,any>)=>{m.payload.counts['water-area']=2;}])await expect(read(await fixture([area],edit))).rejects.toThrow();
 });
 it('snapshots caller buffers before hashing',async()=>{const f=await fixture();const p=read(f);f.manifest.fill(0);f.bytes.fill(0);expect((await p).water).toHaveLength(1);});
 it('preserves unresolved source issues without granting coverage',async()=>{const r=await read(await fixture([],(_m,d)=>{d.source_issues=[{id:'way/9',reason:'missing-node'}];}));expect(r.sourceIssues).toHaveLength(1);expect(r.coverage).toEqual([]);});
});
