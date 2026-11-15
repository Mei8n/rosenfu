import {readWaterEvidencePack,type WaterEvidencePack} from './waterEvidencePack.ts';
import type {WaterPolygon} from './environmentData';
export type WaterEvidenceComposite={status:'incomplete';coverage:[];parts:{key:string;pack:WaterEvidencePack}[];observedAreas:{sourceKey:string;sourceObservationId:string;polygonPart:number;sourceTags:Record<string,string>;polygon:WaterPolygon}[];hasMixedRevisionClaims:boolean;limitations:string[]};
function fail():never{throw Error('水域複合パックが不正・欠損・未対応です。');}
async function hash(bytes:Uint8Array){return [...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes as Uint8Array<ArrayBuffer>))].map(v=>v.toString(16).padStart(2,'0')).join('');}
/** A provenance-preserving inspection view, not a geometry union or construction snapshot. */
export async function readWaterEvidenceComposite(indexBytes:Uint8Array,indexPin:string,files:ReadonlyMap<string,Uint8Array>):Promise<WaterEvidenceComposite>{
 if(!/^[a-f0-9]{64}$/.test(indexPin)||!indexBytes.length||indexBytes.length>65536)fail();
 const indexCopy=new Uint8Array(indexBytes);
 // Snapshot caller-owned files before awaiting any hash; bound total memory first.
 let total=0;const copies=new Map<string,Uint8Array>();
 if(files.size>32)fail();
 for(const [path,bytes] of files){total+=bytes.length;if(total>20*1024*1024)fail();copies.set(path,new Uint8Array(bytes));}
 if(await hash(indexCopy)!==indexPin)fail();
 const m=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(indexCopy));
 if(!m||m.format!=='rosenfu-water-composite'||m.version!==1||m.geographic_completeness!=='unknown'||!Array.isArray(m.coverage)||m.coverage.length||m.source_time_policy!=='retain-per-part-revisions'||!Array.isArray(m.parts)||m.parts.length<2||m.parts.length>16)fail();
 const seen=new Set<string>(),parts:WaterEvidenceComposite['parts']=[],observedAreas:WaterEvidenceComposite['observedAreas']=[];
 let bounds:string|undefined,coordinates=0,observations=0;
 for(const entry of m.parts){
  if(!entry||typeof entry.key!=='string'||!/^[a-z][a-z0-9_-]{0,31}$/.test(entry.key)||seen.has(entry.key)||entry.manifest!==`${entry.key}/manifest.json`||entry.payload!==`${entry.key}/evidence.json`)fail();
  seen.add(entry.key);const manifest=copies.get(entry.manifest),payload=copies.get(entry.payload);if(!manifest||!payload)fail();
  const pack=await readWaterEvidencePack(manifest,entry.manifest_sha256,payload);
  const current=JSON.stringify(pack.bounds);if(bounds!==undefined&&bounds!==current)fail();bounds=current;
  observations+=pack.observations.length;if(observations>20000)fail();
  let areaIndex=0;
  for(const observation of pack.observations){
   const g=observation.geometry;
   coordinates+=g.type==='LineString'?g.coordinates.length:g.type==='Polygon'?g.coordinates.reduce((n,r)=>n+r.length,0):g.coordinates.reduce((n,p)=>n+p.reduce((n,r)=>n+r.length,0),0);
   if(coordinates>200000)fail();
   // Keep the semantic source alongside each split polygon. A basin, covered
   // water or layer tag must not disappear when a consumer selects this view.
   // polygonPart is zero-based within the original observation, not the pack.
   if(g.type!=='LineString')for(let polygonPart=0;polygonPart<(g.type==='Polygon'?1:g.coordinates.length);polygonPart++){
    const polygon=pack.water[areaIndex++];if(!polygon)fail();
    observedAreas.push({sourceKey:entry.key,sourceObservationId:observation.id,polygonPart,sourceTags:{...observation.tags},polygon});
   }
  }
  if(areaIndex!==pack.water.length)fail();
  parts.push({key:entry.key,pack});
 }
 return {status:'incomplete',coverage:[],parts,observedAreas,hasMixedRevisionClaims:new Set(parts.map(p=>p.pack.revision)).size>1,limitations:['No dry-land certification or complete water coverage','Per-part source revisions are claims, not a common verified observation date','Overlaps and duplicate source geometry retained; do not sum lengths or areas','Waterway widths and missing observations remain unresolved']};
}
