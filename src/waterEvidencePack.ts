import type {GraphBounds} from './referenceGraph';
import type {WaterPolygon} from './environmentData';

const MAX_BYTES=5*1024*1024,MAX_FEATURES=10000,MAX_COORDS=100000;
type Kind='water-area'|'waterway-line'|'coastline';
type Point=[number,number];
type Geometry={type:'LineString';coordinates:Point[]}|{type:'Polygon';coordinates:Point[][]}|{type:'MultiPolygon';coordinates:Point[][][]};
export type WaterObservation={id:string;kind:Kind;geometry:Geometry;tags:Record<string,string>};
export type WaterEvidencePack={
 status:'incomplete';revision:string;bounds:GraphBounds;
 /** Always empty: observations never certify dry land or complete water coverage. */
 coverage:GraphBounds[];water:WaterPolygon[];observations:WaterObservation[];
 source:Record<string,unknown>;sourceIssues:{id:string;reason:string}[];limitations:string[];
};
function fail():never{throw Error('水域証拠パックが不正・欠損・未対応です。');}
function object(v:unknown):Record<string,unknown>{if(!v||typeof v!=='object'||Array.isArray(v))fail();return v as Record<string,unknown>;}
function string(v:unknown,max=4096):string{if(typeof v!=='string'||!v.length||[...v].length>max)fail();return v;}
function integer(v:unknown,max:number,min=0):number{if(typeof v!=='number'||!Number.isSafeInteger(v)||v<min||v>max)fail();return v;}
function hash(v:unknown):string{const s=string(v,64);if(!/^[a-f0-9]{64}$/.test(s))fail();return s;}
function unknownCoverage(v:Record<string,unknown>){if(v.geographic_completeness!=='unknown'||!Array.isArray(v.coverage)||v.coverage.length!==0)fail();}
function bounds(v:unknown):GraphBounds{
 if(!Array.isArray(v)||v.length!==4||!v.every(n=>typeof n==='number'&&Number.isFinite(n)))fail();
 const [west,south,east,north]=v;if(west< -180||east>180||south< -85||north>85||west>=east||south>=north||east-west>.2||north-south>.2)fail();
 return {west,south,east,north};
}
async function digest(bytes:Uint8Array):Promise<string>{return [...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes as Uint8Array<ArrayBuffer>))].map(n=>n.toString(16).padStart(2,'0')).join('');}
function decode(bytes:Uint8Array){if(!bytes.length||bytes.length>MAX_BYTES)fail();return object(JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes)));}

/** Read caller-supplied bytes against a separately pinned manifest hash. No network,
 * fallback source, source-date inference or geographic completeness upgrade occurs. */
export async function readWaterEvidencePack(manifestBytes:Uint8Array,expectedManifestSha256:string,payloadBytes:Uint8Array):Promise<WaterEvidencePack>{
 hash(expectedManifestSha256);
 // Snapshot mutable caller buffers before any async boundary (hash/parse TOCTOU).
 if(manifestBytes.length>MAX_BYTES||payloadBytes.length>MAX_BYTES)fail();
 const manifestCopy=new Uint8Array(manifestBytes),payloadCopy=new Uint8Array(payloadBytes);
 if(await digest(manifestCopy)!==expectedManifestSha256)fail();
 const m=decode(manifestCopy),source=object(m.source),entry=object(m.payload);
 if(m.format!=='rosenfu-water-evidence-manifest'||m.version!==1||entry.file!=='evidence.json')fail();unknownCoverage(m);
 const revision=string(m.revision),area=bounds(m.bounds);
 for(const k of ['url','license','license_url','attribution','selection'])string(source[k]);
 for(const k of ['url','license_url']){const u=new URL(source[k] as string);if(u.protocol!=='https:'||u.username||u.password)fail();}
 hash(source.file_sha256);integer(source.bytes,Number.MAX_SAFE_INTEGER,1);
 if(integer(entry.bytes,MAX_BYTES,1)!==payloadCopy.length||hash(entry.sha256)!==await digest(payloadCopy))fail();
 const counts=object(entry.counts),d=decode(payloadCopy);
 for(const k of ['water-area','waterway-line','coastline'])integer(counts[k],MAX_FEATURES);
 if(d.format!=='rosenfu-water-evidence'||d.version!==1||d.revision!==revision||JSON.stringify(bounds(d.bounds))!==JSON.stringify(area))fail();unknownCoverage(d);
 if(!Array.isArray(d.observations)||d.observations.length>MAX_FEATURES||!Array.isArray(d.source_issues)||d.source_issues.length>1000||!Array.isArray(m.limitations)||!m.limitations.length||m.limitations.length>32)fail();
 const seen=new Set<string>(),actual:Record<Kind,number>={'water-area':0,'waterway-line':0,coastline:0};let coordinates=0;
 const water:WaterPolygon[]=[];
 function point(v:unknown):Point{
  if(++coordinates>MAX_COORDS||!Array.isArray(v)||v.length!==2||!v.every(n=>typeof n==='number'&&Number.isFinite(n))||Math.abs(v[0])>180||Math.abs(v[1])>85)fail();return [v[0],v[1]];
 }
 function line(v:unknown,closed:boolean):Point[]{
  if(!Array.isArray(v)||v.length<(closed?4:2))fail();const result=v.map(point);
  if(closed&&(result[0][0]!==result.at(-1)![0]||result[0][1]!==result.at(-1)![1]))fail();return result;
 }
 function polygon(v:unknown):Point[][]{if(!Array.isArray(v)||!v.length)fail();return v.map(r=>line(r,true));}
 const observations=d.observations.map(raw=>{
  const o=object(raw),id=string(o.id,256),kind=o.kind as Kind,g=object(o.geometry),tags=object(o.tags);
  if(!Object.hasOwn(actual,kind)||seen.has(`${kind}:${id}`))fail();seen.add(`${kind}:${id}`);actual[kind]++;
  if(Object.keys(tags).length>128)fail();for(const [k,v] of Object.entries(tags)){string(k,256);if(typeof v!=='string'||[...v].length>4096)fail();}
  let geometry:Geometry;
  if(kind==='water-area'){
   let polygons:Point[][][];
   if(g.type==='Polygon'){const rings=polygon(g.coordinates);polygons=[rings];geometry={type:'Polygon',coordinates:rings};}
   else if(g.type==='MultiPolygon'&&Array.isArray(g.coordinates)&&g.coordinates.length){polygons=g.coordinates.map(polygon);geometry={type:'MultiPolygon',coordinates:polygons};}
   else fail();
   for(const rings of polygons){if(water.length>=MAX_FEATURES)fail();water.push({rings:rings.map(r=>r.map(([longitude,latitude])=>({longitude,latitude})))});}
  }else{if(g.type!=='LineString')fail();geometry={type:'LineString',coordinates:line(g.coordinates,false)};}
  return {id,kind,geometry,tags:tags as Record<string,string>};
 });
 for(const k of ['water-area','waterway-line','coastline'] as const)if(actual[k]!==counts[k])fail();
 return {status:'incomplete',revision,bounds:area,coverage:[],water,observations,source,
  sourceIssues:d.source_issues.map(v=>{const o=object(v);return {id:string(o.id,256),reason:string(o.reason)};}),limitations:m.limitations.map(v=>string(v))};
}
