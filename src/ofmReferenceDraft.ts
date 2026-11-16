import {buildTileRoadPath,type TilePathPick} from './ofmDerivedTilePath';
import type {TileRoadLine} from './ofmBoundaryContinuations';
export type DraftTileLine=TileRoadLine&{extent:number};
export type DraftTileSnapshot={tileLines:readonly DraftTileLine[];coverageVerified?:boolean;bundle:{graph:{dataset:{id:string;revision:string;source:string;license:string};[key:string]:unknown};graphSha256:string;sourceSha256:string;source:{source:string;sourceDate:string|null;license:string;attribution:string}}};
export type DraftTileInput={snapshot:DraftTileSnapshot;rawTileBytes:Uint8Array};
const MAX_RAW=8*1024*1024,MAX_JSON=16*1024*1024;
const fail=(s:string):never=>{throw Error(s);};
const bytes=(s:string)=>new TextEncoder().encode(s);
async function sha(data:Uint8Array){if(!globalThis.crypto?.subtle)fail('sha256-unavailable');return Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',Uint8Array.from(data)))).map(n=>n.toString(16).padStart(2,'0')).join('');}
function base64(data:Uint8Array){let s='';for(let i=0;i<data.length;i+=16384)s+=String.fromCharCode(...data.subarray(i,i+16384));return btoa(s);}
const safe=(s:unknown)=>typeof s==='string'&&s.length>0&&bytes(s).length<=100&&!/[\p{Cc}\p{Cf}]/u.test(s);
/** Isolated draft only. Raw tile evidence is mandatory; no construction/save APIs.
 * SHA establishes integrity, not trust or proof of network completeness. */
export async function prepareOfmReferenceDraft(inputs:readonly DraftTileInput[],requiredTiles:readonly {x:number;y:number}[],start:TilePathPick,end:TilePathPick,reusableEvidenceKeys:readonly string[]=[]){
 if(!inputs.length||inputs.length>16||requiredTiles.length!==inputs.length)fail('tile-context-size');
 if(inputs.reduce((n,t)=>n+t.rawTileBytes.byteLength,0)>MAX_RAW)fail('raw-budget');
 if(inputs.reduce((n,t)=>n+t.snapshot.tileLines.reduce((m,l)=>m+l.coordinates.length,0),0)>100000)fail('context-point-budget');
 let snapshotBytes=0;
 inputs=inputs.map(t=>{const json=JSON.stringify(t.snapshot);snapshotBytes+=bytes(json).length;if(snapshotBytes>MAX_JSON)fail('snapshot-budget');return {snapshot:JSON.parse(json) as DraftTileSnapshot,rawTileBytes:t.rawTileBytes.slice()};});
 requiredTiles=requiredTiles.map(t=>({...t}));start={...start};end={...end};
 if(reusableEvidenceKeys.length>1024||reusableEvidenceKeys.some(k=>!/^([a-f0-9]{64}):([a-f0-9]{64})$/.test(k)))fail('reuse-keys');
 const reusable=new Set(reusableEvidenceKeys);
 const required=new Set(requiredTiles.map(t=>`${t.x}/${t.y}`));
 if(required.size!==requiredTiles.length||requiredTiles.some(t=>![t.x,t.y].every(n=>Number.isInteger(n)&&n>=0&&n<16384)))fail('required-tiles');
 const sources:{key:string;dataset_id:string;revision:string;graph_sha256:string;source_sha256:string;source:string;source_date:null;license:string;attribution:string;conversion_format_version:4}[]=[];
 const context:{sourceKey:string;x:number;y:number;rawTileBase64:string;rawByteLength:number;graphJson:string;tileLines:DraftTileLine[]}[]=[];
 const all:DraftTileLine[]=[];const ids=new Set<string>();let revision:string|undefined;
 for(const {snapshot:s,rawTileBytes:raw}of inputs){
  if(!raw.length||raw.length>5*1024*1024)fail('raw-tile-size');
  const b=s.bundle,source=b.source;
  const match=source.source.match(/^https:\/\/tiles\.openfreemap\.org\/planet\/([A-Za-z0-9_]{1,64})\/14\/(0|[1-9]\d{0,4})\/(0|[1-9]\d{0,4})\.pbf$/);
  if(!match)fail('source-url');const [,rev,xText,yText]=match!;const x=Number(xText),y=Number(yText);
  if(x>=16384||y>=16384||!required.delete(`${x}/${y}`))fail('missing-or-duplicate-tile');
  if(revision!==undefined&&revision!==rev)fail('mixed-revision');revision=rev;
  if(source.sourceDate!==null||source.license!=='ODbL-1.0'||source.attribution!=='© OpenMapTiles © OpenStreetMap contributors'||b.graph.dataset.source!==source.source||b.graph.dataset.revision!==rev||b.graph.dataset.id!==`ofm-tiles/${rev}/14/${x}/${y}`||b.graph.dataset.license!==source.license)fail('source-binding');
  if(!/^[a-f0-9]{64}$/.test(b.sourceSha256)||await sha(raw)!==b.sourceSha256)fail('raw-hash');
  const graphJson=JSON.stringify(b.graph);if(bytes(graphJson).length>MAX_JSON||await sha(bytes(graphJson))!==b.graphSha256)fail('graph-hash');
  if(s.tileLines.length>10000)fail('line-budget');const lines:DraftTileLine[]=[];
  for(const l of s.tileLines){
   const suffix=l.id.slice(`ofm:${rev}:14:${x}:${y}:`.length);
   if(!l.id.startsWith(`ofm:${rev}:14:${x}:${y}:`)||!/^(0|[1-9]\d{0,3}):(0|[1-9]\d{0,8})$/.test(suffix)||ids.has(l.id)||l.revision!==rev||l.x!==x||l.y!==y||l.z!==14)fail('fragment-binding');ids.add(l.id);
   if(!Number.isInteger(l.extent)||l.extent<1||l.extent>65536||l.coordinates.length<2||l.coordinates.length>512||l.coordinates.some(p=>p.length!==2||!p.every(Number.isFinite)||Math.abs(p[0])>180||Math.abs(p[1])>85))fail('geometry');
   if(!safe(l.class)||[l.subclass,l.access,l.brunnel,l.level].some(v=>v!==undefined&&!safe(v))||(l.layer!==undefined&&!Number.isFinite(l.layer))||(l.oneway!==undefined&&!(typeof l.oneway==='number'?Number.isFinite(l.oneway):safe(l.oneway))))fail('attributes');
   const copied:DraftTileLine=JSON.parse(JSON.stringify(l));lines.push(copied);all.push(copied);
  }
  const graphNodes=b.graph.nodes,graphEdges=b.graph.edges;
  if(!Array.isArray(graphNodes)||!Array.isArray(graphEdges))fail('graph-context');
  const nodes=graphNodes as {osmId:string;longitude:number;latitude:number}[],edges=graphEdges as {id:string;osmWayId:string;segmentIndex:number;from:string;to:string;mode:string}[];
  const expectedNodes=new Map<string,readonly [number,number]>(lines.flatMap(l=>l.coordinates.map((p,i)=>[`${l.id}:${i}`,p] as const)));
  const expectedEdges=new Map(lines.flatMap(l=>l.coordinates.slice(1).flatMap((p,i)=>p[0]===l.coordinates[i][0]&&p[1]===l.coordinates[i][1]?[]:[[`${l.id}/${i}`,{way:l.id,index:i,from:`${l.id}:${i}`,to:`${l.id}:${i+1}`}]])));
  if(nodes.length!==expectedNodes.size||edges.length!==expectedEdges.size)fail('incomplete-context');
  for(const n of nodes){if(!n||typeof n!=='object')fail('node-context');const p=expectedNodes.get(n.osmId);if(!p||p[0]!==n.longitude||p[1]!==n.latitude)fail('node-context');expectedNodes.delete(n.osmId);}
  for(const e of edges){if(!e||typeof e!=='object')fail('edge-context');const expected=expectedEdges.get(e.id);if(!expected||e.osmWayId!==expected.way||e.segmentIndex!==expected.index||e.from!==expected.from||e.to!==expected.to||e.mode!=='road')fail('edge-context');expectedEdges.delete(e.id);}
  const key=b.graphSha256+':'+b.sourceSha256;
  sources.push({key,dataset_id:b.graph.dataset.id,revision:rev,graph_sha256:b.graphSha256,source_sha256:b.sourceSha256,source:source.source,source_date:null,license:source.license,attribution:source.attribution,conversion_format_version:4});
  context.push({sourceKey:key,x,y,rawTileBase64:base64(raw),rawByteLength:raw.length,graphJson,tileLines:lines});
 }
 if(required.size)fail('missing-tile');
 const candidateClasses=new Set(['primary','secondary','tertiary','minor','service']);
 const path=buildTileRoadPath(all.filter(l=>candidateClasses.has(l.class)),start,end);
 const sourceByFragment=new Map(context.flatMap(t=>t.tileLines.map(l=>[l.id,t.sourceKey] as const)));
 const payload={format:'rosenfu-ofm-reference-draft' as const,version:1 as const,method:'ofm_tile_path_v1' as const,geometryDerived:true as const,coverageVerified:false as const,decoderRule:'ofm_transportation_z14_v1' as const,requiredTiles:requiredTiles.map(t=>({...t})),start:{...start},end:{...end},requiredSourceKeys:sources.map(s=>s.key),newSources:sources.filter(s=>!reusable.has(s.key)),candidateContextKeys:context.map(t=>t.sourceKey),newEvidence:context.filter(t=>!reusable.has(t.sourceKey)),path:{...path,orderedSpans:path.orderedSpans.map(s=>({...s,sourceKey:sourceByFragment.get(s.fragmentId)!}))}};
 if(bytes(JSON.stringify(payload)).length>MAX_JSON)fail('payload-budget');return payload;
}
