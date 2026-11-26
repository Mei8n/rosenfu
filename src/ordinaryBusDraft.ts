import type {GeoPoint} from './alignment';
import {project,type Camera} from './map';
import {OFM_ATTRIBUTION,OFM_EVIDENCE_LIMITS,OFM_EVIDENCE_RULE,OpenFreeMapTimeoutCooldown,roadTileBounds,type OpenFreeMapEvidenceRequest,type OpenFreeMapRoadLine} from './openFreeMapRoadData';
import type {RoadSnapshot} from './automaticRoadData';
import type {CurrentRoadEvidenceResult} from './useAutomaticRoad';
import type {RawTileEvidence,TileEvidenceBinding} from './ofmSharedEvidenceDraft';

type Vertex={feature:number;line:number;vertex:number};
type Tile={binding:OpenFreeMapEvidenceRequest;lines:OpenFreeMapRoadLine[]};
export type BusAnchor={point:GeoPoint;raw:Vertex;tile:OpenFreeMapEvidenceRequest;fractionQ?:number};
type TileProof={tileKey:string;tileRawSha256:string};
export type OrdinaryBusGradePolicy='observedConnectors';
export type OrdinaryBusPattern='openRoundTrip'|'closedOneWay';
export type BusInteriorStop={id:string;name:string;occurrenceKey:string;fractionQ:number};
export type OrdinaryBusStopView={id:string;stopId:string;name:string;point:GeoPoint;servesReturn:boolean};
export type BusWaypoint={id:string;anchor:BusAnchor};
type BusWaypointProof={id:string;tileKey:string;vertex:Vertex};
export type OrdinaryBusPreviewReply=OrdinaryBusPreview|{points:[];lengthM:0;failedLegIndex:number};
export type OrdinaryBusPath=TileProof & {respectOneway:boolean;start:Vertex;end:Vertex;pattern?:OrdinaryBusPattern;gradePolicy?:OrdinaryBusGradePolicy;via?:BusWaypointProof[];interiorStops?:BusInteriorStop[]} & (
 {rule:'ofm_ground_bus_z14_v1';sameAtomStops?:readonly [number,number];fractionalTerminals?:readonly [number,number]}|
 {rule:'ofm_ground_bus_rectangle_z14_v2'|'ofm_ground_bus_shared_seam_z14_v3';endTileKey:string;additionalTiles:TileProof[];sameAtomStops?:never;fractionalTerminals?:readonly [number,number]});
type RawTilePair={rawEvidence:RawTileEvidence;tileBinding:TileEvidenceBinding};
export type RouteTileObserver=(cells:readonly (readonly [number,number])[],revision:string,signal?:AbortSignal)=>Promise<OpenFreeMapEvidenceRequest[]>;
export type OrdinaryBusQuery={expectedWorldToken:string;routeId:string;name:string;path:OrdinaryBusPath;rawEvidence:RawTileEvidence|null;tileBinding:TileEvidenceBinding|null;contextTiles?:RawTilePair[]};
export type OrdinaryBusPreviewQuery=OrdinaryBusQuery & {requestStopTargets?:true;requestAutoStopSuggestions?:true};
type BusLegPreview={points:GeoPoint[];lengthM:number};
export type OrdinaryBusPreview=BusLegPreview & {returnPath?:BusLegPreview;returnUnavailableReason?:string;viaPointIndices?:number[];stopOccurrenceKeys?:string[];stopTargetTerminalCuts?:readonly [number,number];interiorStops?:OrdinaryBusStopView[];autoStopSuggestions?:BusInteriorStop[];loopFallbackStop?:OrdinaryBusStopView};
export type BusEvidenceReader=(refs:readonly OpenFreeMapEvidenceRequest[],signal?:AbortSignal)=>Promise<CurrentRoadEvidenceResult>;
const supported=new Set(['primary','secondary','tertiary','minor','service','busway','bus_guideway']);
const access=new Set(['yes','permissive','designated']);
const oneway=new Set([0,1,-1,'0','1','-1','no','yes']);
const territory=(tile:OpenFreeMapEvidenceRequest)=>`https://tiles.openfreemap.org/planet/${tile.revision}/14/${tile.x}/${tile.y}.pbf`;
const sha=(value:string)=>/^[a-f0-9]{64}$/.test(value);
const validIdx=(n:number)=>Number.isSafeInteger(n)&&n>=0&&n<16384;
const sameTile=(a:OpenFreeMapEvidenceRequest,b:OpenFreeMapEvidenceRequest)=>territory(a)===territory(b)&&a.rawSha256===b.rawSha256;
const finitePoint=(p:GeoPoint)=>Number.isFinite(p.latitude)&&Number.isFinite(p.longitude)&&Math.abs(p.latitude)<=85.05112878&&Math.abs(p.longitude)<=180;
const BUS_CUT_DEN=1_000_000;
function observedBusLine(l:OpenFreeMapRoadLine,binding:OpenFreeMapEvidenceRequest){
 if(l.revision!==binding.revision||l.z!==14||l.x!==binding.x||l.y!==binding.y||
    !supported.has(l.class)||l.brunnel!==undefined||l.layer!==undefined&&l.layer!==0||
    l.access!==undefined&&!access.has(l.access)||l.oneway!==undefined&&!oneway.has(l.oneway))return;
 const prefix=`ofm:${binding.revision}:14:${binding.x}:${binding.y}:`;
 if(!l.id.startsWith(prefix))return;
 const match=l.id.slice(prefix.length).match(/^(0|[1-9]\d*):(0|[1-9]\d*)$/);
 if(!match||!match.slice(1).every(n=>Number.isSafeInteger(Number(n))&&Number(n)<=0xffffffff))return;
 return {feature:Number(match[1]),line:Number(match[2])};
}
const sameAtom=(a:BusAnchor,b:BusAnchor)=>sameTile(a.tile,b.tile)&&a.raw.feature===b.raw.feature&&a.raw.line===b.raw.line&&a.raw.vertex===b.raw.vertex;
/** This resolves a display hit only. The directed usable atom comes from the native solver. */
export function snapOrdinaryBusSegment(snapshot:RoadSnapshot|undefined,at:GeoPoint,camera:Camera,scale:number):BusAnchor{
 if(!finitePoint(at)||!Number.isFinite(scale)||scale<=0)throw Error('地図上の道路を選んでください。');
 const tiles=tilesOf(snapshot);
 if(tiles.length>16)throw Error('道路が多すぎます。地図を拡大してください。');
 const p=project(at.latitude,at.longitude,camera),radius=24*360/(scale*256*2**camera.zoom);
 let best:{anchor:BusAnchor;distance:number}|undefined,ambiguous=false,seen=0;
 for(const {binding,lines} of tiles){
  const cell=roadTileBounds(binding.x,binding.y);
  const inside=({latitude,longitude}:GeoPoint)=>longitude>cell.west&&longitude<cell.east&&latitude<cell.north&&latitude>cell.south;
  for(const l of lines){
   seen+=l.coordinates.length;
   if(seen>160_000)throw Error('道路が多すぎます。地図を拡大してください。');
   const id=observedBusLine(l,binding);
   if(!id)continue;
   for(let vertex=0;vertex+1<l.coordinates.length;vertex++){
    const [[x,y],[u,v]]=[l.coordinates[vertex],l.coordinates[vertex+1]];
    if(at.longitude<Math.min(x,u)-radius||at.longitude>Math.max(x,u)+radius||
       at.latitude<Math.min(y,v)-radius||at.latitude>Math.max(y,v)+radius)continue;
    const a={longitude:x,latitude:y},b={longitude:u,latitude:v};
    if(!finitePoint(a)||!finitePoint(b)||!inside(a)||!inside(b))continue;
    const aa=project(y,x,camera),bb=project(v,u,camera),dx=bb.x-aa.x,dy=bb.y-aa.y;
    const len2=dx*dx+dy*dy;
    if(len2===0)continue;
    const q=Math.round(((p.x-aa.x)*dx+(p.y-aa.y)*dy)/len2*BUS_CUT_DEN);
    if(q<=0||q>=BUS_CUT_DEN)continue;
    const t=q/BUS_CUT_DEN;
    const distance=Math.hypot(p.x-aa.x-t*dx,p.y-aa.y-t*dy)*scale;
    if(distance>24)continue;
    const point={longitude:x+(u-x)*t,latitude:y+(v-y)*t};
    const next={anchor:{tile:binding,point,raw:{...id,vertex},fractionQ:q},distance};
    if(!best||distance<best.distance-0.001){best=next;ambiguous=false;}
    else if(Math.abs(distance-best.distance)<=0.001&&!sameAtom(next.anchor,best.anchor))ambiguous=true;
   }
  }
 }
 if(ambiguous)throw Error('道路が重なっています。地図を拡大して選び直してください。');
 if(!best)throw Error('道路の途中を選んでください。交差点・タイルの境界は指定できません。');
 return best.anchor;
}


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
function snapOrdinaryBusChoices(snapshot:RoadSnapshot|undefined,at:GeoPoint,camera:Camera,scale:number,previous?:BusAnchor,rectangle=false,nearby?:readonly BusAnchor[]):BusAnchor[]{
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
   const id=observedBusLine(l,binding);
   if(!id)continue;
   for(let vertex=0;vertex<l.coordinates.length;vertex++){
    const [longitude,latitude]=l.coordinates[vertex],point={longitude,latitude};
    if(!finitePoint(point)||longitude<cell.west||longitude>=cell.east||latitude>cell.north||latitude<=cell.south||Math.abs(at.longitude-longitude)>geoRadius||Math.abs(at.latitude-latitude)>geoRadius)continue;
    const q=project(latitude,longitude,camera),d=Math.hypot(p.x-q.x,p.y-q.y)*scale;
    if(d>24)continue;
    candidates.push({anchor:{point,raw:{...id,vertex},tile:binding},distance:d});
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
 const out=[anchor];
 if(nearby){
  // A different cell is eligible only if replacing the free endpoint
  // keeps the exact rectangle defined by all fixed control points.
  const fixed=nearby.map(v=>v.tile);
  const grid=fixed.length&&fixed.every(t=>t.revision===anchor.tile.revision)?
   ordinaryBusTileFootprint(fixed[0],fixed[0],fixed.slice(1)):undefined;
  const sameRectangle=(tile:OpenFreeMapEvidenceRequest)=>grid&&tile.revision===anchor.tile.revision&&
   Math.min(grid.west,tile.x)===Math.min(grid.west,anchor.tile.x)&&Math.max(grid.east,tile.x)===Math.max(grid.east,anchor.tile.x)&&
   Math.min(grid.north,tile.y)===Math.min(grid.north,anchor.tile.y)&&Math.max(grid.south,tile.y)===Math.max(grid.south,anchor.tile.y);
  for(const other of candidates){
   if(out.length>=3)break;
   if((sameTile(other.anchor.tile,anchor.tile)||sameRectangle(other.anchor.tile))&&
      !out.some(p=>samePlace(p.point,other.anchor.point)))out.push(other.anchor);
  }
 }
 return out;
}
export function snapOrdinaryBusVertex(snapshot:RoadSnapshot|undefined,at:GeoPoint,camera:Camera,scale:number,previous?:BusAnchor,rectangle=false):BusAnchor{
 return snapOrdinaryBusChoices(snapshot,at,camera,scale,previous,rectangle)[0];
}
/** Up to two distinct alternatives from the same confirmed rectangle. */
export function snapOrdinaryBusFreeEnd(snapshot:RoadSnapshot|undefined,at:GeoPoint,camera:Camera,scale:number,fixed:readonly BusAnchor[]=[]):BusAnchor[]{
 return snapOrdinaryBusChoices(snapshot,at,camera,scale,undefined,true,fixed);
}
export function hasObservedOrdinaryBusTile(snapshot:RoadSnapshot|undefined,tile:OpenFreeMapEvidenceRequest):boolean{
 return tilesOf(snapshot).some(t=>sameTile(t.binding,tile));
}
/** Bridge a worker's 20m ray pick to the observed tile digest. This is a
 * bounded source anchor, not a second projection or asserted native topology. */
export function observedTramRoadAnchor(
 snapshot:RoadSnapshot|undefined,pick:{fragmentId:string;segmentIndex:number;fraction:number;point:GeoPoint},
):{anchor:BusAnchor;fractionQ:number}{
 const matches=tilesOf(snapshot).flatMap(({binding,lines})=>lines.filter(l=>l.id===pick.fragmentId).map(line=>({binding,line})));
 const seen=matches[0],index=pick.segmentIndex;
 if(matches.length!==1||!seen||!Number.isSafeInteger(index)||index<0||index>=seen.line.coordinates.length-1
    ||!Number.isFinite(pick.fraction)||pick.fraction<0||pick.fraction>1)throw Error('道路上の点を選び直してください。');
 const {binding,line}=seen;
 const prefix=`ofm:${binding.revision}:14:${binding.x}:${binding.y}:`;
 const m=line.id.startsWith(prefix)?line.id.slice(prefix.length).match(/^(0|[1-9]\d*):(0|[1-9]\d*)$/):null;
 if(!m||line.x!==binding.x||line.y!==binding.y||line.revision!==binding.revision||!m.slice(1).every(n=>Number.isSafeInteger(Number(n))&&Number(n)<=0xffffffff))
    throw Error('選択した道路の元データが変わりました。');
 return {anchor:{tile:binding,point:pick.point,raw:{feature:Number(m[1]),line:Number(m[2]),vertex:index}},fractionQ:Math.round(pick.fraction*1_000_000)};
}
const sameRawAnchor=(a:BusAnchor,b:BusAnchor)=>sameTile(a.tile,b.tile)&&a.raw.feature===b.raw.feature&&a.raw.line===b.raw.line&&a.raw.vertex===b.raw.vertex;
const samePlace=(a:GeoPoint,b:GeoPoint)=>Math.abs(a.latitude-b.latitude)<1e-12&&Math.abs(a.longitude-b.longitude)<1e-12;

export function ordinaryBusTileFootprint(start:OpenFreeMapEvidenceRequest,end:OpenFreeMapEvidenceRequest,via:readonly OpenFreeMapEvidenceRequest[]=[]){
 const anchors=[start,...via,end];
 if(anchors.some(t=>t.revision!==start.revision))throw Error('道路の版が変わりました。位置を選び直してください。');
 if(anchors.some(t=>!validIdx(t.x)||!validIdx(t.y)||t.z!==14))throw Error('道路の位置が不正です。');
 const xs=anchors.map(t=>t.x),ys=anchors.map(t=>t.y);
 const west=Math.min(...xs),east=Math.max(...xs),north=Math.min(...ys),south=Math.max(...ys);
 return {west,east,north,south,count:(east-west+1)*(south-north+1)};
}

export function ordinaryBusCells(start:OpenFreeMapEvidenceRequest,end:OpenFreeMapEvidenceRequest,via:readonly OpenFreeMapEvidenceRequest[]=[]):[number,number][]{
 const {west,east,north,south,count}=ordinaryBusTileFootprint(start,end,via);
 const limit=OFM_EVIDENCE_LIMITS.batchTiles;
 if(count>limit)throw Error(`バス経路の確認範囲が広すぎます（${count} タイル必要・上限 ${limit} タイル）。起点・終点・経由点を近づけてください。`);
 const cells:[number,number][]=[];
 for(let x=west;x<=east;x++)for(let y=north;y<=south;y++)cells.push([x,y]);
 return cells;
}

export async function prepareOrdinaryBusQuery(
 start:BusAnchor,end:BusAnchor,name:string,routeId:string,expectedWorldToken:string,read:BusEvidenceReader,
 descriptors:readonly unknown[],signal?:AbortSignal,observe?:RouteTileObserver,pattern?:OrdinaryBusPattern,via:readonly BusWaypoint[]=[],respectOneway=false,interiorStops:readonly BusInteriorStop[]=[],gradePolicy?:OrdinaryBusGradePolicy,allowGeneralFractional=false,
):Promise<OrdinaryBusQuery>{
 const fractional=start.fractionQ!==undefined||end.fractionQ!==undefined;
 let sameAtomStops:readonly [number,number]|undefined;
 let fractionalTerminals:readonly [number,number]|undefined;
 let rawStart=start.raw,rawEnd=end.raw;
 if(fractional){
  const [from,to]=[start.fractionQ,end.fractionQ];
  const valid=(q:number|undefined):q is number=>q!==undefined&&Number.isSafeInteger(q)&&q>0&&q<BUS_CUT_DEN;
  if(!valid(from)||!valid(to)||start.raw.vertex>=0xffffffff||end.raw.vertex>=0xffffffff||
     pattern!==undefined||via.length||(!allowGeneralFractional&&interiorStops.length)||gradePolicy)
   throw Error('道路の途中にある起点と終点を選んでください。');
  if(allowGeneralFractional){
   fractionalTerminals=[from,to];
  }else{
   if(from===to||!sameAtom(start,end))throw Error('起点と終点は同じ道路区間の異なる位置を選んでください。');
   const tail={...start.raw,vertex:start.raw.vertex+1};
   if(from<to){rawEnd=tail;sameAtomStops=[from,to];}
   else{rawStart=tail;rawEnd=start.raw;sameAtomStops=[BUS_CUT_DEN-from,BUS_CUT_DEN-to];}
  }
 }
 const anchors=[start,...via.map(w=>w.anchor),end];
 if(pattern==='closedOneWay'?
    !via.length||!sameRawAnchor(start,end)||!samePlace(start.point,end.point):
    samePlace(start.point,end.point))throw Error('異なる道路位置を選んでください。');
 if(anchors.some((a,i)=>i>0&&samePlace(anchors[i-1].point,a.point)))throw Error('連続する点は別の道路位置を選んでください。');
 if(via.length>30||new Set(via.map(w=>w.id)).size!==via.length||via.some(w=>!/^[-_A-Za-z0-9]{1,80}$/.test(w.id)))throw Error('経由点の指定が不正です。');
 const a=start.tile,b=end.tile;
 const base={...(fractionalTerminals?{fractionalTerminals}:{}),...(gradePolicy?{gradePolicy}:{}),...(interiorStops.length?{interiorStops:[...interiorStops]}:{}),tileKey:territory(a),tileRawSha256:a.rawSha256,respectOneway,start:rawStart,end:rawEnd,...(pattern?{pattern}:{}),...(via.length?{via:via.map(w=>({id:w.id,tileKey:territory(w.anchor.tile),vertex:w.anchor.raw}))}:{})};
 let refs=[a],path:OrdinaryBusPath={...base,rule:'ofm_ground_bus_z14_v1',...(sameAtomStops?{sameAtomStops}:{})};
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
 return attachOrdinaryBusEvidence(out,refs,read,descriptors,signal);
}
async function attachOrdinaryBusEvidence(query:OrdinaryBusQuery,refs:OpenFreeMapEvidenceRequest[],read:BusEvidenceReader,descriptors:readonly unknown[],signal?:AbortSignal){
 const pairs=await readObservedRoadTilePairs(refs,read,descriptors,signal);
 return !pairs.length?query:query.path.rule==='ofm_ground_bus_z14_v1'?{...query,...pairs[0]}:{...query,contextTiles:pairs};
}

/** One extra bounded query, using only complete rectangles from the same observed revision. */
export async function prepareNearbyOrdinaryBusQuery(
 query:OrdinaryBusQuery,snapshot:RoadSnapshot|undefined,read:BusEvidenceReader,descriptors:readonly unknown[],signal?:AbortSignal,
):Promise<OrdinaryBusQuery|undefined>{
 const path=query.path;
 if(signal?.aborted||path.rule!=='ofm_ground_bus_z14_v1'||path.sameAtomStops||path.fractionalTerminals)return;
 const visible=tilesOf(snapshot).map(t=>t.binding);
 if(visible.length>OFM_EVIDENCE_LIMITS.batchTiles)return;
 const first=visible.find(t=>territory(t)===path.tileKey&&t.rawSha256===path.tileRawSha256);
 if(!first)return;
 const cells=new Map<string,OpenFreeMapEvidenceRequest>();
 for(const t of visible){
  if(t.revision!==first.revision||Math.abs(t.x-first.x)>1||Math.abs(t.y-first.y)>1)continue;
  const key=t.x+'/'+t.y;
  if(cells.has(key))return;
  cells.set(key,t);
 }
 let best:OpenFreeMapEvidenceRequest[]=[];
 for(let west=0;west<=1;west++)for(let east=0;east<=1;east++)
 for(let north=0;north<=1;north++)for(let south=0;south<=1;south++){
  const count=(west+east+1)*(north+south+1);
  if(count<=Math.max(1,best.length))continue;
  const candidate:OpenFreeMapEvidenceRequest[]=[];
  for(let x=first.x-west;x<=first.x+east;x++)for(let y=first.y-north;y<=first.y+south;y++){
   const tile=cells.get(x+'/'+y);
   if(tile)candidate.push(tile);
  }
  if(candidate.length===count)best=candidate;
 }
 if(!best.length)return;
 const {sameAtomStops:_,...base}=path;
 const revised:OrdinaryBusPath={...base,rule:'ofm_ground_bus_rectangle_z14_v2',endTileKey:path.tileKey,
  additionalTiles:best.filter(t=>t!==first).map(t=>({tileKey:territory(t),tileRawSha256:t.rawSha256}))};
 return attachOrdinaryBusEvidence({...query,path:revised,rawEvidence:null,tileBinding:null},best,read,descriptors,signal);
}

/** Prefer the old rule. A typed no-route result may reuse the exact same pinned bytes with the new rule. */
export function retryOrdinaryBusSharedSeam(query:OrdinaryBusQuery):OrdinaryBusQuery|undefined{
 if(query.path.rule!=='ofm_ground_bus_rectangle_z14_v2'||query.path.fractionalTerminals)return;
 return {...query,path:{...query.path,rule:'ofm_ground_bus_shared_seam_z14_v3'}};
}

export class BusSeamReadTimeout extends Error{
 constructor(){super('道路の再取得が時間内に終わりませんでした。選んだ経路はそのままです。1分ほどおいて「バス系統を確定」を押すと再試行できます。');}
}

/** Confirm reacquires only the exact proofs checked by the preview, not a
 * rectangle derived from the camera. It may re-observe a panned-away cell,
 * but only through the same observed revision and with the same byte hash. */
export async function reacquireNearbyOrdinaryBusQuery(
 query:OrdinaryBusQuery,snapshot:RoadSnapshot|undefined,read:BusEvidenceReader,descriptors:readonly unknown[],signal?:AbortSignal,observe?:RouteTileObserver,
):Promise<OrdinaryBusQuery>{
 if(query.path.rule==='ofm_ground_bus_z14_v1')throw Error('経路の確認範囲が変わりました。');
 const changed=()=>Error('道路の元データが変わりました。位置を選び直してください。');
 const proofs=[query.path,...query.path.additionalTiles];
 if(proofs.length>OFM_EVIDENCE_LIMITS.batchTiles||new Set(proofs.map(p=>p.tileKey)).size!==proofs.length)throw changed();
 const refs=proofs.map(proof=>{
  const match=proof.tileKey.match(/^https:\/\/tiles\.openfreemap\.org\/planet\/([A-Za-z0-9_]{1,64})\/14\/(\d{1,5})\/(\d{1,5})\.pbf$/);
  if(!match||!validIdx(Number(match[2]))||!validIdx(Number(match[3]))||!sha(proof.tileRawSha256))throw changed();
  const ref:OpenFreeMapEvidenceRequest={revision:match[1],z:14,x:Number(match[2]),y:Number(match[3]),rawSha256:proof.tileRawSha256,decoderRule:OFM_EVIDENCE_RULE};
  if(territory(ref)!==proof.tileKey)throw changed();
  return ref;
 });
 const revision=refs[0].revision;
 if(signal?.aborted||refs.some(ref=>ref.revision!==revision))throw changed();
 const visible=tilesOf(snapshot).map(t=>t.binding);
 const missing=refs.filter(ref=>{
  const sameCell=visible.filter(t=>t.x===ref.x&&t.y===ref.y);
  if(sameCell.length>1||sameCell.length===1&&!sameTile(ref,sameCell[0]))throw changed();
  // Already retained by this world: native confirmation will reconstruct
  // this exact pin. Don't charge the live provider for an offscreen copy.
  return sameCell.length===0&&!retainedRoadTile(ref,descriptors);
 });
 if(missing.length){
  if(!observe)throw changed();
  let observed:OpenFreeMapEvidenceRequest[];
  try{observed=await observe(missing.map(({x,y})=>[x,y]),revision,signal);}
  catch(e){
   // Only an actual transport/queue deadline during this read can be
   // retried with the existing preview. Not cancellation, a changed
   // provider revision, nor a similarly worded native commit error.
   if(!signal?.aborted&&(e instanceof OpenFreeMapTimeoutCooldown||e instanceof DOMException&&e.name==='TimeoutError'))throw new BusSeamReadTimeout();
   throw changed();
  }
  if(signal?.aborted||observed.length!==missing.length||missing.some(ref=>observed.filter(t=>t.z===14&&t.decoderRule===OFM_EVIDENCE_RULE&&sameTile(t,ref)).length!==1))throw changed();
 }
 return attachOrdinaryBusEvidence(query,refs,read,descriptors,signal);
}
function retainedRoadTile(tile:OpenFreeMapEvidenceRequest,descriptors:readonly unknown[]):boolean{
 return descriptors.some(item=>{
  if(!item||typeof item!=='object'||!('binding' in item)||!item.binding||typeof item.binding!=='object')return false;
  const binding=item.binding as Record<string,unknown>;
  return binding.canonical_url===territory(tile)&&binding.raw_sha256===tile.rawSha256;
 });
}
export async function readObservedRoadTilePairs(refs:OpenFreeMapEvidenceRequest[],read:BusEvidenceReader,descriptors:readonly unknown[],signal?:AbortSignal):Promise<RawTilePair[]>{
 // Retained descriptors are hints; Rust still validates the chosen scope and bytes.
 const toFetch=refs.filter(t=>!retainedRoadTile(t,descriptors));
 if(!toFetch.length)return [];
 const result=await read(toFetch,signal);
 if(signal?.aborted||result.status==='stale')throw Error('道路が更新されました。位置を選び直してください。');
 if(result.status==='unavailable')throw Error('道路データを確認できませんでした。拡大するか、もう一度お試しください。');
 const {rawTiles,bindings}=result.evidence;
 const rawBySha=new Map(rawTiles.map(blob=>[blob.sha256,blob]));
 const byKey=new Map(bindings.map(binding=>[binding.key,binding]));
 const needed=new Set(toFetch.map(t=>t.rawSha256));
 if(bindings.length!==toFetch.length||byKey.size!==toFetch.length||rawTiles.length!==needed.size||rawBySha.size!==needed.size)throw Error('道路の元データが揃っていません。');
 let sum=0;
 return toFetch.map(t=>{
  const tileKey=territory(t),blob=rawBySha.get(t.rawSha256),b=byKey.get(tileKey);
  if(!blob||!b||b.url!==tileKey||!sameTile(t,b)||b.z!==14||b.decoderRule!==OFM_EVIDENCE_RULE||b.license!=='ODbL-1.0'||b.attribution!==OFM_ATTRIBUTION||
     !blob.bytes.length||blob.bytes.length>OFM_EVIDENCE_LIMITS.batchBytes)throw Error('選択した道路のデータが変わりました。');
  sum+=blob.bytes.length;
  if(sum>OFM_EVIDENCE_LIMITS.batchBytes)throw Error('道路の元データ量が上限を超えています。');
  const chunks:string[]=[];
  for(let offset=0;offset<blob.bytes.length;offset+=8190)chunks.push(btoa(String.fromCharCode(...blob.bytes.subarray(offset,offset+8190))));
  return {tileBinding:b,rawEvidence:{sha256:t.rawSha256,byteLength:blob.bytes.length,base64:chunks.join('')}};
 });
}
export type AuthoredTramChainGuide={
 tiles:TileProof[];start:Vertex;end:Vertex;startFractionQ:number;endFractionQ:number;
};
/** Only a single shortest cardinal cell order already observed in this view.
 * The backend independently reconstructs every seam from the bound bytes. */
function observedTramChain(snapshot:RoadSnapshot|undefined,start:OpenFreeMapEvidenceRequest,end:OpenFreeMapEvidenceRequest){
 const manhattan=(a:OpenFreeMapEvidenceRequest,b:OpenFreeMapEvidenceRequest)=>Math.abs(a.x-b.x)+Math.abs(a.y-b.y);
 const hops=manhattan(start,end);
 if(hops<2||hops>3||start.revision!==end.revision)throw Error('併用軌道は近くの連続した道路範囲で選んでください。');
 const observed=tilesOf(snapshot);
 if(observed.length>OFM_EVIDENCE_LIMITS.batchTiles)throw Error('道路の確認範囲が広すぎます。');
 const byCell=new Map<string,OpenFreeMapEvidenceRequest>();
 const cellKey=(x:number,y:number)=>x+'/'+y;
 for(const {binding:t} of observed){
  if(t.x<Math.min(start.x,end.x)||t.x>Math.max(start.x,end.x)||
     t.y<Math.min(start.y,end.y)||t.y>Math.max(start.y,end.y))continue;
  const key=cellKey(t.x,t.y);
  if(t.revision!==start.revision||byCell.has(key))throw Error('道路の版が変わりました。位置を選び直してください。');
  byCell.set(key,t);
 }
 if(!sameTile(start,byCell.get(cellKey(start.x,start.y))??end)||
    !sameTile(end,byCell.get(cellKey(end.x,end.y))??start))
  throw Error('選択した道路のデータが変わりました。');
 const paths:OpenFreeMapEvidenceRequest[][]=[];
 function step(path:OpenFreeMapEvidenceRequest[]){
  if(paths.length>1)return;
  const last=path.at(-1)!,remaining=manhattan(last,end);
  if(!remaining){paths.push(path);return;}
  for(const [x,y] of [[last.x-1,last.y],[last.x+1,last.y],[last.x,last.y-1],[last.x,last.y+1]]){
   const t=byCell.get(cellKey(x,y));
   if(t&&manhattan(t,end)===remaining-1)step([...path,t]);
  }
 }
 step([start]);
 if(paths.length!==1)throw Error(paths.length
  ?'道路範囲の経由が複数あります。区間を分けて選び直してください。'
  :'途中の道路データが揃っていません。道路を表示してから選び直してください。');
 return paths[0];
}
export async function prepareAuthoredTramChainQuery(
 snapshot:RoadSnapshot|undefined,from:ReturnType<typeof observedTramRoadAnchor>,to:ReturnType<typeof observedTramRoadAnchor>,
 expectedWorldToken:string,read:BusEvidenceReader,descriptors:readonly unknown[],
){
 const refs=observedTramChain(snapshot,from.anchor.tile,to.anchor.tile);
 const contextTiles=await readObservedRoadTilePairs(refs,read,descriptors);
 const chainGuide:AuthoredTramChainGuide={
  tiles:refs.map(t=>({tileKey:territory(t),tileRawSha256:t.rawSha256})),
  start:from.anchor.raw,end:to.anchor.raw,startFractionQ:from.fractionQ,endFractionQ:to.fractionQ,
 };
 return {expectedWorldToken,chainGuide,rawEvidence:null,tileBinding:null,...(contextTiles.length?{contextTiles}:{})};
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
 const records=[path,...(path.rule!=='ofm_ground_bus_z14_v1'?path.additionalTiles:[])];
 const anchor=(key:string,raw:Vertex,point:GeoPoint):BusAnchor=>{
  const record=records.find(t=>t.tileKey===key);
  if(!record||!point)throw Error('保存済みの道路を確認できません。');
  return {tile:parseTile(key,record.tileRawSha256),raw,point};
 };
 const ids=new Set(path.via?.map(c=>c.id)??[]);
 const freshId=()=>{const id=`b-${crypto.randomUUID()}`;if(ids.has(id))throw Error('経由点の識別子が重なっています。');ids.add(id);return id;};
 const first=anchor(path.tileKey,path.start,result.points[0]),last=anchor(path.rule!=='ofm_ground_bus_z14_v1'?path.endTileKey:path.tileKey,path.end,result.points.at(-1)!);
 const controls=[{id:freshId(),anchor:first},...(path.via??[]).map((v,i)=>({id:v.id,anchor:anchor(v.tileKey,v.vertex,result.points[result.viaPointIndices?.[i]??-1])})),{id:freshId(),anchor:last}];
 const matched=matchOrdinaryBusPreview(result,first,last,path.pattern,controls.map(c=>c.anchor));
 if(!('preview' in matched))throw Error('保存済みのバス経路を確認できません。');
 return {controls,preview:matched.preview};
}


/** A hover or a refused free endpoint may reuse immutable click evidence.
 * An alternate start can change the address roles, never the tile scope. */
export function cachedBusHoverQuery(prior:OrdinaryBusQuery,controls:readonly BusWaypoint[],allowStartRebind=false):OrdinaryBusQuery|undefined{
 if(controls.length<2||controls.length>32||(prior.path.sameAtomStops||prior.path.fractionalTerminals))return;
 const [first,last]=[controls[0].anchor,controls.at(-1)!.anchor],previous=prior.path;
 const startKey=territory(first.tile);
 const newStart=startKey!==previous.tileKey;
 if((newStart&&(!allowStartRebind||previous.rule==='ofm_ground_bus_z14_v1'))||
   (previous.pattern==='closedOneWay'?!sameRawAnchor(first,last):samePlace(first.point,last.point))||controls.some((c,i)=>i>0&&samePlace(controls[i-1].anchor.point,c.anchor.point)))return;
 const proof=[previous,...(previous.rule!=='ofm_ground_bus_z14_v1'?previous.additionalTiles:[])];
 if(controls.some(c=>!proof.some(p=>p.tileKey===territory(c.anchor.tile)&&p.tileRawSha256===c.anchor.tile.rawSha256)))return;
 let cells:[number,number][];
 try{cells=ordinaryBusCells(first.tile,last.tile,controls.slice(1,-1).map(c=>c.anchor.tile));}
 catch{return;}
 if(cells.length!==proof.length||cells.some(([x,y])=>!proof.some(p=>p.tileKey===territory({...first.tile,x,y}))))return;
 const {via:_,...base}=previous;
 const via=controls.slice(1,-1).map(({id,anchor})=>({id,tileKey:territory(anchor.tile),vertex:anchor.raw}));
 const path:OrdinaryBusPath={...base,start:first.raw,end:last.raw,...(via.length?{via}:{})};
 if(path.rule!=='ofm_ground_bus_z14_v1'){
  path.endTileKey=territory(last.tile);
  if(newStart){
   path.tileKey=startKey;path.tileRawSha256=first.tile.rawSha256;
   path.additionalTiles=cells.flatMap(([x,y])=>{
    const k=territory({...first.tile,x,y});
    if(k===startKey)return [];
    const tile=proof.find(p=>p.tileKey===k)!; // every cell was verified above
    return [{tileKey:k,tileRawSha256:tile.tileRawSha256}];
   });
  }
 }
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
