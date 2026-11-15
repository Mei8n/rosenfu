import {it,expect} from 'vitest';
import {readWaterEvidenceComposite} from './waterEvidenceComposite';
import type {WaterObservation} from './waterEvidencePack';
const encode=(v:unknown)=>new TextEncoder().encode(JSON.stringify(v));
async function hash(b:Uint8Array){return [...new Uint8Array(await crypto.subtle.digest('SHA-256',b as Uint8Array<ArrayBuffer>))].map(v=>v.toString(16).padStart(2,'0')).join('');}
async function fixture(observations:WaterObservation[]=[]){
 const files=new Map<string,Uint8Array>();const parts=[];
 for(const key of ['inland','marine']){
  const d={format:'rosenfu-water-evidence',version:1,revision:key==='inland'?'2026-11-14':'2026-11-13',bounds:[134,34,134.1,34.1],geographic_completeness:'unknown',coverage:[],observations,source_issues:[]};
  const payload=encode(d);const manifest=encode({format:'rosenfu-water-evidence-manifest',version:1,revision:d.revision,bounds:d.bounds,geographic_completeness:'unknown',coverage:[],source:{url:'https://source.test',file_sha256:'a'.repeat(64),bytes:1,license:'ODbL-1.0',license_url:'https://license.test',attribution:'OSM',selection:'test'},payload:{file:'evidence.json',bytes:payload.length,sha256:await hash(payload),counts:Object.fromEntries(['water-area','waterway-line','coastline'].map(kind=>[kind,observations.filter(o=>o.kind===kind).length]))},limitations:['No completeness']});
  files.set(`${key}/manifest.json`,manifest);files.set(`${key}/evidence.json`,payload);parts.push({key,manifest:`${key}/manifest.json`,payload:`${key}/evidence.json`,manifest_sha256:await hash(manifest)});
 }
 const index={format:'rosenfu-water-composite',version:1,geographic_completeness:'unknown',coverage:[],source_time_policy:'retain-per-part-revisions',parts};
 return {files,index};
}
async function read(f:Awaited<ReturnType<typeof fixture>>){const bytes=encode(f.index);return readWaterEvidenceComposite(bytes,await hash(bytes),f.files);}
it('retains differing source claims and empty parts as unknown',async()=>{const r=await read(await fixture());expect(r.parts.map(p=>p.pack.revision)).toEqual(['2026-11-14','2026-11-13']);expect(r.hasMixedRevisionClaims).toBe(true);expect(r.status).toBe('incomplete');expect(r.coverage).toEqual([]);});
it('rejects missing or changed part instead of silently loading the remainder',async()=>{let f=await fixture();f.files.delete('marine/evidence.json');await expect(read(f)).rejects.toThrow();f=await fixture();f.files.get('marine/evidence.json')![0]=32;await expect(read(f)).rejects.toThrow();});
it('rejects forged completeness and unsafe paths even with valid index hash',async()=>{let f=await fixture();f.index.geographic_completeness='complete';await expect(read(f)).rejects.toThrow();f=await fixture();f.index.parts[0].key='../inland';await expect(read(f)).rejects.toThrow();});
it('rejects a duplicate part',async()=>{const f=await fixture();f.index.parts[1]=f.index.parts[0];await expect(read(f)).rejects.toThrow();});
it('snapshots index and part bytes before asynchronous checks',async()=>{const f=await fixture(),bytes=encode(f.index),pin=await hash(bytes);const pending=readWaterEvidenceComposite(bytes,pin,f.files);bytes.fill(0);for(const b of f.files.values())b.fill(0);expect((await pending).parts).toHaveLength(2);});
it('keeps eight regional sources and marine separate even when bytes overlap',async()=>{const f=await fixture(),first=f.index.parts[0],parts=[];for(let i=0;i<8;i++){const key=`inland_${i}`;f.files.set(`${key}/manifest.json`,f.files.get(first.manifest)!);f.files.set(`${key}/evidence.json`,f.files.get(first.payload)!);parts.push({...first,key,manifest:`${key}/manifest.json`,payload:`${key}/evidence.json`});}parts.push(f.index.parts[1]);f.files.delete(first.manifest);f.files.delete(first.payload);f.index.parts=parts;const r=await read(f);expect(r.parts).toHaveLength(9);expect(r.coverage).toEqual([]);expect(r.status).toBe('incomplete');expect(r.hasMixedRevisionClaims).toBe(true);});

it('keeps source identity, raw semantic tags and multipart indices on each observed area',async()=>{
 const ring=[[134.01,34.01],[134.09,34.01],[134.09,34.09],[134.01,34.01]] as [number,number][];
 const hole=[[134.03,34.03],[134.04,34.03],[134.04,34.04],[134.03,34.03]] as [number,number][];
 const second=ring.map(([x,y])=>[x+.1,y] as [number,number]);
 const observations:WaterObservation[]=[
  {id:'way/1',kind:'water-area',tags:{landuse:'basin',basin:'detention'},geometry:{type:'Polygon',coordinates:[ring,hole]}},
  {id:'way/2',kind:'waterway-line',tags:{waterway:'stream'},geometry:{type:'LineString',coordinates:ring.slice(0,2)}},
  {id:'relation/1',kind:'water-area',tags:{natural:'water',covered:'yes',tunnel:'culvert',layer:'-1'},geometry:{type:'MultiPolygon',coordinates:[[ring],[second]]}},
 ];
 const r=await read(await fixture(observations));
 expect(r.observedAreas.map(a=>[a.sourceKey,a.sourceObservationId,a.polygonPart])).toEqual([
  ['inland','way/1',0],['inland','relation/1',0],['inland','relation/1',1],
  ['marine','way/1',0],['marine','relation/1',0],['marine','relation/1',1],
 ]);
 expect(r.observedAreas[0].sourceTags).toEqual(observations[0].tags);
 expect(r.observedAreas[0].polygon.rings).toHaveLength(2);
 for(const index of [1,2,4,5])expect(r.observedAreas[index].sourceTags).toEqual(observations[2].tags);
 expect(r.observedAreas[2].polygon.rings[0][0]).toEqual({longitude:second[0][0],latitude:second[0][1]});
 expect(r.status).toBe('incomplete');expect(r.coverage).toEqual([]);
});
