import type {GeoPoint} from './alignment';
import {project,type Camera} from './map';
import {OFM_ATTRIBUTION,OFM_EVIDENCE_LIMITS,OFM_EVIDENCE_RULE,roadTileBounds,type OpenFreeMapEvidenceRequest,type OpenFreeMapRoadLine} from './openFreeMapRoadData';
import type {RoadSnapshot} from './automaticRoadData';
import type {CurrentRoadEvidenceResult} from './useAutomaticRoad';
import type {RawTileEvidence,TileEvidenceBinding} from './ofmSharedEvidenceDraft';

type Vertex={feature:number;line:number;vertex:number};
type Tile={binding:OpenFreeMapEvidenceRequest;lines:OpenFreeMapRoadLine[]};
export type BusAnchor={point:GeoPoint;raw:Vertex;tile:OpenFreeMapEvidenceRequest};
type TileProof={tileKey:string;tileRawSha256:string};
export type OrdinaryBusGradePolicy='observedConnectors';
export type OrdinaryBusPattern='openRoundTrip'|'closedOneWay';
export type BusInteriorStop={id:string;name:string;occurrenceKey:string;fractionQ:number};
export type OrdinaryBusStopView={id:string;stopId:string;name:string;point:GeoPoint;servesReturn:boolean};
export type BusWaypoint={id:string;anchor:BusAnchor};
type BusWaypointProof={id:string;tileKey:string;vertex:Vertex};
export type OrdinaryBusPreviewReply=OrdinaryBusPreview|{points:[];lengthM:0;failedLegIndex:number};
export type OrdinaryBusPath=TileProof & {respectOneway:boolean;start:Vertex;end:Vertex;pattern?:OrdinaryBusPattern;gradePolicy?:OrdinaryBusGradePolicy;via?:BusWaypointProof[];interiorStops?:BusInteriorStop[]} & (
 {rule:'ofm_ground_bus_z14_v1'}|{rule:'ofm_ground_bus_rectangle_z14_v2';endTileKey:string;additionalTiles:TileProof[]});
type RawTilePair={rawEvidence:RawTileEvidence;tileBinding:TileEvidenceBinding};
export type RouteTileObserver=(cells:readonly (readonly [number,number])[],revision:string,signal?:AbortSignal)=>Promise<OpenFreeMapEvidenceRequest[]>;
export type OrdinaryBusQuery={expectedWorldToken:string;routeId:string;name:string;path:OrdinaryBusPath;rawEvidence:RawTileEvidence|null;tileBinding:TileEvidenceBinding|null;contextTiles?:RawTilePair[]};
export type OrdinaryBusPreviewQuery=OrdinaryBusQuery & {requestStopTargets?:true;requestAutoStopSuggestions?:true};
type BusLegPreview={points:GeoPoint[];lengthM:number};
export type OrdinaryBusPreview=BusLegPreview & {returnPath?:BusLegPreview;returnUnavailableReason?:string;viaPointIndices?:number[];stopOccurrenceKeys?:string[];interiorStops?:OrdinaryBusStopView[];autoStopSuggestions?:BusInteriorStop[];loopFallbackStop?:OrdinaryBusStopView};
export type BusEvidenceReader=(refs:readonly OpenFreeMapEvidenceRequest[],signal?:AbortSignal)=>Promise<CurrentRoadEvidenceResult>;
const supported=new Set(['primary','secondary','tertiary','minor','service','busway','bus_guideway']);
const access=new Set(['yes','permissive','designated']);
const oneway=new Set([0,1,-1,'0','1','-1','no','yes']);
const territory=(tile:OpenFreeMapEvidenceRequest)=>`https://tiles.openfreemap.org/planet/${tile.revision}/14/${tile.x}/${tile.y}.pbf`;
const sha=(value:string)=>/^[a-f0-9]{64}$/.test(value);
const validIdx=(n:number)=>Number.isSafeInteger(n)&&n>=0&&n<16384;
const sameTile=(a:OpenFreeMapEvidenceRequest,b:OpenFreeMapEvidenceRequest)=>territory(a)===territory(b)&&a.rawSha256===b.rawSha256;
const finitePoint=(p:GeoPoint)=>Number.isFinite(p.latitude)&&Number.isFinite(p.longitude)&&Math.abs(p.latitude)<=85.05112878&&Math.abs(p.longitude)<=180;

function tilesOf(snapshot?:RoadSnapshot):Tile[]{
 if(!snapshot)return [];
 return (snapshot.tileSnapshots??[]).flatMap(tile=>{
  const m=tile.bundle.source.source.match(/^https:\/\/tiles\.openfreemap\.org\/planet\/([A-Za-z0-9_]{1,64})\/14\/(\d+)\/(\d+)\.pbf$/);
  if(!m||!sha(tile.bundle.sourceSha256)||!validIdx(Number(m[2]))||!validIdx(Number(m[3]))||!('tileLines' in tile)||!Array.isArray(tile.tileLines))return [];
  const [,revision,x,y]=m;
  return [{binding:{z:14,revision,x:Number(x),y:Number(y),rawSha256:tile.bundle.sourceSha256,decoderRule:OFM_EVIDENCE_RULE},lines:tile.tileLines as OpenFreeMapRoadLine[]}];
 });
}

/** The raw native anchors are real MVT vertices, not invented segment splits.
 * Query only the currently observed tiles. A colocated ground junction can
 * expose several source descriptors; use one stable original index because
 * the native graph uses the exact same encoded vertex. Distinct equally near
 * placements and different copies of a tile-boundary point are not guessed. */
export function snapOrdinaryBusVertex(snapshot:RoadSnapshot|undefined,at:GeoPoint,camera:Camera,scale:number,previous?:BusAnchor,rectangle=false):BusAnchor{
 if(!finitePoint(at)||!Number.isFinite(scale)||scale<=0)throw Error('地図上の道路を選んでください。');
 const tiles=tilesOf(snapshot);let seen=0;
 const fromView=tiles.some(item=>previous&&sameTile(item.binding,previous.tile));
 if(previous&&!rectangle&&!fromView)throw Error('起点を置いた範囲の道路を選んでください。');
 if(tiles.length>16)throw Error('道路が多すぎます。地図を拡大してください。');
 const p=project(at.latitude,at.longitude,camera);
 const geoRadius=24*360/(scale*256*2**camera.zoom);
 const candidates:{anchor:BusAnchor;distance:number}[]=[];
 // Include other observed tiles here so a click outside the chosen tile
 // cannot silently land on a visible neighbour inside the old one.
 for(const {binding,lines} of tiles){
  const cell=roadTileBounds(binding.x,binding.y);
  for(const l of lines){
   seen+=l.coordinates.length;
   if(seen>160_000)throw Error('道路が多すぎます。地図を拡大してください。');
   if(l.revision!==binding.revision||l.z!==14||l.x!==binding.x||l.y!==binding.y||
      !supported.has(l.class)||l.brunnel!==undefined||l.layer!==undefined&&l.layer!==0||
      l.access!==undefined&&!access.has(l.access)||l.oneway!==undefined&&!oneway.has(l.oneway))continue;
   const prefix=`ofm:${binding.revision}:14:${binding.x}:${binding.y}:`;
   if(!l.id.startsWith(prefix))continue;
   const match=l.id.slice(prefix.length).match(/^(0|[1-9]\d*):(0|[1-9]\d*)$/);
   if(!match||!match.slice(1).every(n=>Number.isSafeInteger(Number(n))&&Number(n)<=0xffffffff))continue;
   for(let vertex=0;vertex<l.coordinates.length;vertex++){
    const [longitude,latitude]=l.coordinates[vertex],point={longitude,latitude};
    if(!finitePoint(point)||longitude<cell.west||longitude>=cell.east||latitude>cell.north||latitude<=cell.south||Math.abs(at.longitude-longitude)>geoRadius||Math.abs(at.latitude-latitude)>geoRadius)continue;
    const q=project(latitude,longitude,camera),d=Math.hypot(p.x-q.x,p.y-q.y)*scale;
    if(d>24)continue;
    candidates.push({anchor:{point,raw:{feature:Number(match[1]),line:Number(match[2]),vertex},tile:binding},distance:d});
   }
  }
 }
 candidates.sort((a,b)=>a.distance-b.distance||a.anchor.raw.feature-b.anchor.raw.feature||a.anchor.raw.line-b.anchor.raw.line||a.anchor.raw.vertex-b.anchor.raw.vertex);
 const nearest=candidates[0];
 if(!nearest)throw Error('道路上の点を見つけられません。道路の曲がり角などに近づけて選んでください。');
 const {anchor}=nearest;
 const related=candidates.filter(c=>c.distance<=nearest.distance+0.001);
 if(previous&&!rectangle&&!sameTile(anchor.tile,previous.tile))throw Error('起点と終点は同じ道路の取得範囲に置いてください。範囲をまたぐ経路は未対応です。');
 if(related.some(c=>!sameTile(c.anchor.tile,anchor.tile)||!samePlace(c.anchor.point,anchor.point)))throw Error('位置が重なっています。地図を拡大して選び直してください。');
 return anchor;
}
const sameRawAnchor=(a:BusAnchor,b:BusAnchor)=>sameTile(a.tile,b.tile)&&a.raw.feature===b.raw.feature&&a.raw.line===b.raw.line&&a.raw.vertex===b.raw.vertex;
const samePlace=(a:GeoPoint,b:GeoPoint)=>Math.abs(a.latitude-b.latitude)<1e-12&&Math.abs(a.longitude-b.longitude)<1e-12;

export function ordinaryBusCells(start:OpenFreeMapEvidenceRequest,end:OpenFreeMapEvidenceRequest,via:readonly OpenFreeMapEvidenceRequest[]=[]):[number,number][]{
 const anchors=[start,...via,end];
 if(anchors.some(t=>t.revision!==start.revision))throw Error('道路の版が変わりました。位置を選び直してください。');
 if(anchors.some(t=>!validIdx(t.x)||!validIdx(t.y)||t.z!==14))throw Error('道路の位置が不正です。');
 const xs=anchors.map(t=>t.x),ys=anchors.map(t=>t.y);
 const west=Math.min(...xs),east=Math.max(...xs),north=Math.min(...ys),south=Math.max(...ys);
 if((east-west+1)*(south-north+1)>OFM_EVIDENCE_LIMITS.batchTiles)throw Error('バス経路の確認範囲が広すぎます。近くの道路を選び直してください。');
 const cells:[number,number][]=[];
 for(let x=west;x<=east;x++)for(let y=north;y<=south;y++)cells.push([x,y]);
 return cells;
}

export async function prepareOrdinaryBusQuery(
 start:BusAnchor,end:BusAnchor,name:string,routeId:string,expectedWorldToken:string,read:BusEvidenceReader,
 descriptors:readonly unknown[],signal?:AbortSignal,observe?:RouteTileObserver,pattern?:OrdinaryBusPattern,via:readonly BusWaypoint[]=[],respectOneway=false,interiorStops:readonly BusInteriorStop[]=[],gradePolicy?:OrdinaryBusGradePolicy,
):Promise<OrdinaryBusQuery>{
 const anchors=[start,...via.map(w=>w.anchor),end];
 if(pattern==='closedOneWay'?
    !via.length||!sameRawAnchor(start,end)||!samePlace(start.point,end.point):
    samePlace(start.point,end.point))throw Error('異なる道路位置を選んでください。');
 if(anchors.some((a,i)=>i>0&&samePlace(anchors[i-1].point,a.point)))throw Error('連続する点は別の道路位置を選んでください。');
 if(via.length>30||new Set(via.map(w=>w.id)).size!==via.length||via.some(w=>!/^[-_A-Za-z0-9]{1,80}$/.test(w.id)))throw Error('経由点の指定が不正です。');
 const a=start.tile,b=end.tile;
 const base={...(gradePolicy?{gradePolicy}:{}),...(interiorStops.length?{interiorStops:[...interiorStops]}:{}),tileKey:territory(a),tileRawSha256:a.rawSha256,respectOneway,start:start.raw,end:end.raw,...(pattern?{pattern}:{}),...(via.length?{via:via.map(w=>({id:w.id,tileKey:territory(w.anchor.tile),vertex:w.anchor.raw}))}:{})};
 let refs=[a],path:OrdinaryBusPath={...base,rule:'ofm_ground_bus_z14_v1'};
 if(anchors.some(w=>!sameTile(a,w.tile))){
  if(!observe)throw Error('経由点を含む道路データを確認できません。');
  const cells=ordinaryBusCells(a,b,via.map(w=>w.anchor.tile));
  if(cells.length===1&&anchors.some(p=>!sameTile(p.tile,a)))throw Error('道路データが変わりました。位置を選び直してください。');
  let observed:OpenFreeMapEvidenceRequest[];
  try{observed=await observe(cells,a.revision,signal);}
  catch{throw Error('範囲内の道路データを揃えられませんでした。位置を選び直してください。');}
  if(signal?.aborted||observed.length!==cells.length)throw Error('範囲内の道路データが揃っていません。');
  const refByCell=new Map(observed.map(t=>[`${t.x}/${t.y}`,t]));
  refs=cells.map(([x,y])=>{
   const t=refByCell.get(`${x}/${y}`);
   if(!t||t.z!==14||t.revision!==a.revision||t.decoderRule!==OFM_EVIDENCE_RULE||!sha(t.rawSha256))throw Error('範囲内の道路データが揃っていません。');
   return t;
  });
  if(anchors.some(w=>!refs.some(t=>sameTile(t,w.tile))))throw Error('選択した道路のデータが変わりました。');
  path={...base,rule:'ofm_ground_bus_rectangle_z14_v2',endTileKey:territory(b),additionalTiles:refs.filter(t=>!sameTile(t,a)).map(t=>({tileKey:territory(t),tileRawSha256:t.rawSha256}))};
 }
 const out:OrdinaryBusQuery={expectedWorldToken,routeId,name,path,rawEvidence:null,tileBinding:null};
 // Retained descriptors are hints; Rust still validates the chosen scope and bytes.
 const toFetch=refs.filter(t=>!descriptors.some(item=>{
  if(!item||typeof item!=='object'||!('binding' in item)||!item.binding||typeof item.binding!=='object')return false;
  const b=item.binding as Record<string,unknown>;
  return b.canonical_url===territory(t)&&b.raw_sha256===t.rawSha256;
 }));
 if(!toFetch.length)return out;
 const result=await read(toFetch,signal);
 if(signal?.aborted||result.status==='stale')throw Error('道路が更新されました。位置を選び直してください。');
 if(result.status==='unavailable')throw Error('道路データを確認できませんでした。拡大するか、もう一度お試しください。');
 const {rawTiles,bindings}=result.evidence;
 const rawBySha=new Map(rawTiles.map(blob=>[blob.sha256,blob]));
 const byKey=new Map(bindings.map(binding=>[binding.key,binding]));
 const needed=new Set(toFetch.map(t=>t.rawSha256));
 if(bindings.length!==toFetch.length||byKey.size!==toFetch.length||rawTiles.length!==needed.size||rawBySha.size!==needed.size)throw Error('道路の元データが揃っていません。');
 let sum=0;
 const pairs=toFetch.map(t=>{
  const tileKey=territory(t),blob=rawBySha.get(t.rawSha256),b=byKey.get(tileKey);
  if(!blob||!b||b.url!==tileKey||!sameTile(t,b)||b.z!==14||b.decoderRule!==OFM_EVIDENCE_RULE||b.license!=='ODbL-1.0'||b.attribution!==OFM_ATTRIBUTION||
     !blob.bytes.length||blob.bytes.length>OFM_EVIDENCE_LIMITS.batchBytes)throw Error('選択した道路のデータが変わりました。');
  sum+=blob.bytes.length;
  if(sum>OFM_EVIDENCE_LIMITS.batchBytes)throw Error('道路の元データ量が上限を超えています。');
  const chunks:string[]=[];
  for(let offset=0;offset<blob.bytes.length;offset+=8190)chunks.push(btoa(String.fromCharCode(...blob.bytes.subarray(offset,offset+8190))));
  return {tileBinding:b,rawEvidence:{sha256:t.rawSha256,byteLength:blob.bytes.length,base64:chunks.join('')}};
 });
 return path.rule==='ofm_ground_bus_z14_v1'?{...out,...pairs[0]}:{...out,contextTiles:pairs};
}
function matchLeg(result:BusLegPreview,start:BusAnchor,end:BusAnchor){
 return result&&Array.isArray(result.points)&&result.points.length>=2&&result.points.length<=4096&&result.points.every(finitePoint)&&
    Number.isFinite(result.lengthM)&&result.lengthM>0&&samePlace(result.points[0],start.point)&&samePlace(result.points.at(-1)!,end.point);
}
export function matchOrdinaryBusPreview(
 result:OrdinaryBusPreviewReply,start:BusAnchor,end:BusAnchor,pattern:OrdinaryBusPattern|undefined,controls:readonly BusAnchor[],
):{preview:OrdinaryBusPreview}|{refusedLeg:number}{
 if(result&&'failedLegIndex' in result){
  if(controls.length<2||!Number.isInteger(result.failedLegIndex)||result.failedLegIndex<0||result.failedLegIndex>=controls.length-1||
     !Array.isArray(result.points)||result.points.length!==0||result.lengthM!==0||'returnPath' in result||'returnUnavailableReason' in result)
   throw Error('経路の確認結果が一致しません。位置を選び直してください。');
  return {refusedLeg:result.failedLegIndex};
 }
 const preview=verifyOrdinaryBusPreview(result,start,end,pattern),vias=controls.slice(1,-1);
 if(vias.length){
  const indices=preview.viaPointIndices;
  if(!Array.isArray(indices)||indices.length!==vias.length||indices.some((i,n)=>!Number.isInteger(i)||
      i<1||i>=preview.points.length-1||n>0&&i<=indices[n-1]||!samePlace(preview.points[i],vias[n].point)))
   throw Error('経由点の確認結果が一致しません。位置を選び直してください。');
 }else if(preview.viaPointIndices!==undefined&&(!Array.isArray(preview.viaPointIndices)||preview.viaPointIndices.length!==0))
  throw Error('経由点の確認結果が一致しません。位置を選び直してください。');
 return {preview};
}
export function verifyOrdinaryBusPreview(result:OrdinaryBusPreview,start:BusAnchor,end:BusAnchor,pattern?:OrdinaryBusPattern):OrdinaryBusPreview{
 const error=()=>Error('経路の確認結果が一致しません。位置を選び直してください。');
 if(!matchLeg(result,start,end))throw error();
 if(pattern==='openRoundTrip'){
  if(result.returnPath){
   if(result.returnUnavailableReason!==undefined||!matchLeg(result.returnPath,end,start))throw error();
  }else if(typeof result.returnUnavailableReason!=='string'||!result.returnUnavailableReason.trim()||result.returnUnavailableReason.length>500)throw error();
 }else if(result.returnPath!==undefined||result.returnUnavailableReason!==undefined)throw error();
 const fallback=result.loopFallbackStop;
 if(pattern==='closedOneWay'){
  if(!sameRawAnchor(start,end)||!samePlace(start.point,end.point))throw error();
  if(fallback&&(!fallback.point||!finitePoint(fallback.point)||fallback.id!=='loop_fallback'||!fallback.stopId||!fallback.name||fallback.servesReturn!==false))throw error();
 }else if(fallback!==undefined)throw error();
 return result;
}


/** A saved native proof is reopened by immutable tile descriptors and its
 * fresh native output, never by guessing which road lies under drawn geometry. */
export function restoreOrdinaryBusControls(path:OrdinaryBusPath,result:OrdinaryBusPreviewReply):{controls:BusWaypoint[];preview:OrdinaryBusPreview}{
 if(!result||'failedLegIndex' in result||!Array.isArray(result.points)||result.points.length<2)throw Error('保存済みのバス経路を確認できません。');
 const parseTile=(key:string,rawSha256:string):OpenFreeMapEvidenceRequest=>{
  const m=key.match(/^https:\/\/tiles\.openfreemap\.org\/planet\/([A-Za-z0-9_]{1,64})\/14\/(\d+)\/(\d+)\.pbf$/);
  if(!m||!sha(rawSha256)||!validIdx(Number(m[2]))||!validIdx(Number(m[3])))throw Error('保存済みの道路を確認できません。');
  return {z:14,revision:m[1],x:Number(m[2]),y:Number(m[3]),rawSha256,decoderRule:OFM_EVIDENCE_RULE};
 };
 const records=[path,...(path.rule==='ofm_ground_bus_rectangle_z14_v2'?path.additionalTiles:[])];
 const anchor=(key:string,raw:Vertex,point:GeoPoint):BusAnchor=>{
  const record=records.find(t=>t.tileKey===key);
  if(!record||!point)throw Error('保存済みの道路を確認できません。');
  return {tile:parseTile(key,record.tileRawSha256),raw,point};
 };
 const ids=new Set(path.via?.map(c=>c.id)??[]);
 const freshId=()=>{const id=`b-${crypto.randomUUID()}`;if(ids.has(id))throw Error('経由点の識別子が重なっています。');ids.add(id);return id;};
 const first=anchor(path.tileKey,path.start,result.points[0]),last=anchor(path.rule==='ofm_ground_bus_rectangle_z14_v2'?path.endTileKey:path.tileKey,path.end,result.points.at(-1)!);
 const controls=[{id:freshId(),anchor:first},...(path.via??[]).map((v,i)=>({id:v.id,anchor:anchor(v.tileKey,v.vertex,result.points[result.viaPointIndices?.[i]??-1])})),{id:freshId(),anchor:last}];
 const matched=matchOrdinaryBusPreview(result,first,last,path.pattern,controls.map(c=>c.anchor));
 if(!('preview' in matched))throw Error('保存済みのバス経路を確認できません。');
 return {controls,preview:matched.preview};
}


/** A hover can use only the immutable evidence already prepared by a click.
 * It cannot widen the tile scope, touch the provider, or fabricate a leg. */
export function cachedBusHoverQuery(prior:OrdinaryBusQuery,controls:readonly BusWaypoint[]):OrdinaryBusQuery|undefined{
 if(controls.length<2||controls.length>32)return;
 const [first,last]=[controls[0].anchor,controls.at(-1)!.anchor],previous=prior.path;
 if(territory(first.tile)!==previous.tileKey||first.tile.rawSha256!==previous.tileRawSha256||
   (previous.pattern==='closedOneWay'?!sameRawAnchor(first,last):samePlace(first.point,last.point))||controls.some((c,i)=>i>0&&samePlace(controls[i-1].anchor.point,c.anchor.point)))return;
 const proof=[previous,...(previous.rule==='ofm_ground_bus_rectangle_z14_v2'?previous.additionalTiles:[])];
 if(controls.some(c=>!proof.some(p=>p.tileKey===territory(c.anchor.tile)&&p.tileRawSha256===c.anchor.tile.rawSha256)))return;
 let cells:[number,number][];
 try{cells=ordinaryBusCells(first.tile,last.tile,controls.slice(1,-1).map(c=>c.anchor.tile));}
 catch{return;}
 if(cells.length!==proof.length||cells.some(([x,y])=>!proof.some(p=>p.tileKey===territory({...first.tile,x,y}))))return;
 const {via:_,...base}=previous;
 const via=controls.slice(1,-1).map(({id,anchor})=>({id,tileKey:territory(anchor.tile),vertex:anchor.raw}));
 const path:OrdinaryBusPath={...base,start:first.raw,end:last.raw,...(via.length?{via}:{})};
 if(path.rule==='ofm_ground_bus_rectangle_z14_v2')path.endTileKey=territory(last.tile);
 return {...prior,path};
}


/** Fresh queries and memoized queries need not have the same JS key
 * insertion order; binding membership and point order still must match. */
export function sameOrdinaryBusPath(a:OrdinaryBusPath,b:OrdinaryBusPath):boolean{
 const canonical=(value:unknown):unknown=>Array.isArray(value)?value.map(canonical):
  value&&typeof value==='object'?Object.fromEntries(Object.entries(value).sort(([x],[y])=>x<y?-1:x>y?1:0).map(([k,v])=>[k,canonical(v)])):value;
 return JSON.stringify(canonical(a))===JSON.stringify(canonical(b));
}


export function sameOrdinaryBusGeometry(a:OrdinaryBusPreview,b:OrdinaryBusPreview):boolean{
 const equal=(x:BusLegPreview,y:BusLegPreview)=>x.lengthM===y.lengthM&&x.points.length===y.points.length&&
  x.points.every((point,i)=>samePlace(point,y.points[i]));
 return equal(a,b)&&(!a.returnPath&&!b.returnPath||!!a.returnPath&&!!b.returnPath&&equal(a.returnPath,b.returnPath));
}
