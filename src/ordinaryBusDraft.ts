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
export type OrdinaryBusPattern='openRoundTrip';
export type OrdinaryBusPath=TileProof & {respectOneway:boolean;start:Vertex;end:Vertex;pattern?:OrdinaryBusPattern} & (
 {rule:'ofm_ground_bus_z14_v1'}|{rule:'ofm_ground_bus_rectangle_z14_v2';endTileKey:string;additionalTiles:TileProof[]});
type RawTilePair={rawEvidence:RawTileEvidence;tileBinding:TileEvidenceBinding};
export type RouteTileObserver=(cells:readonly (readonly [number,number])[],revision:string,signal?:AbortSignal)=>Promise<OpenFreeMapEvidenceRequest[]>;
export type OrdinaryBusQuery={expectedWorldToken:string;routeId:string;name:string;path:OrdinaryBusPath;rawEvidence:RawTileEvidence|null;tileBinding:TileEvidenceBinding|null;contextTiles?:RawTilePair[]};
type BusLegPreview={points:GeoPoint[];lengthM:number};
export type OrdinaryBusPreview=BusLegPreview & {returnPath?:BusLegPreview;returnUnavailableReason?:string};
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
const samePlace=(a:GeoPoint,b:GeoPoint)=>Math.abs(a.latitude-b.latitude)<1e-12&&Math.abs(a.longitude-b.longitude)<1e-12;

export function ordinaryBusCells(start:OpenFreeMapEvidenceRequest,end:OpenFreeMapEvidenceRequest):[number,number][]{
 if(start.revision!==end.revision)throw Error('道路の版が変わりました。位置を選び直してください。');
 if(![start.x,start.y,end.x,end.y].every(validIdx)||start.z!==14||end.z!==14)throw Error('道路の位置が不正です。');
 const west=Math.min(start.x,end.x),east=Math.max(start.x,end.x),north=Math.min(start.y,end.y),south=Math.max(start.y,end.y);
 if((east-west+1)*(south-north+1)>OFM_EVIDENCE_LIMITS.batchTiles)throw Error('バス経路の確認範囲が広すぎます。起点と終点を近づけてください。');
 const cells:[number,number][]=[];
 for(let x=west;x<=east;x++)for(let y=north;y<=south;y++)cells.push([x,y]);
 return cells;
}

export async function prepareOrdinaryBusQuery(
 start:BusAnchor,end:BusAnchor,name:string,routeId:string,expectedWorldToken:string,read:BusEvidenceReader,
 descriptors:readonly unknown[],signal?:AbortSignal,observe?:RouteTileObserver,pattern?:OrdinaryBusPattern,
):Promise<OrdinaryBusQuery>{
 if(samePlace(start.point,end.point))throw Error('異なる道路位置を選んでください。');
 const a=start.tile,b=end.tile;
 const base={tileKey:territory(a),tileRawSha256:a.rawSha256,respectOneway:false,start:start.raw,end:end.raw,...(pattern?{pattern}:{})};
 let refs=[a],path:OrdinaryBusPath={...base,rule:'ofm_ground_bus_z14_v1'};
 if(!sameTile(a,b)){
  if(!observe)throw Error('この版では同じ取得範囲内の道路を選んでください。');
  const cells=ordinaryBusCells(a,b);
  if(cells.length===1)throw Error('道路データが変わりました。位置を選び直してください。');
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
  if(!refs.some(t=>sameTile(t,a))||!refs.some(t=>sameTile(t,b)))throw Error('選択した道路のデータが変わりました。');
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
export function verifyOrdinaryBusPreview(result:OrdinaryBusPreview,start:BusAnchor,end:BusAnchor,pattern?:OrdinaryBusPattern):OrdinaryBusPreview{
 const error=()=>Error('経路の確認結果が一致しません。位置を選び直してください。');
 if(!matchLeg(result,start,end))throw error();
 if(pattern==='openRoundTrip'){
  if(result.returnPath){
   if(result.returnUnavailableReason!==undefined||!matchLeg(result.returnPath,end,start))throw error();
  }else if(typeof result.returnUnavailableReason!=='string'||!result.returnUnavailableReason.trim()||result.returnUnavailableReason.length>500)throw error();
 }else if(result.returnPath!==undefined||result.returnUnavailableReason!==undefined)throw error();
 return result;
}
