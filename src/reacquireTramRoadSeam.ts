/** Confirmed-click recovery only: the previous authored anchor can leave the
 * viewport after panning. Observe only its one missing neighboring Z14 tile. */
import {distanceM} from './alignment';
import {observedTramRoadAnchor} from './ordinaryBusDraft';
import type {RoadSnapshot} from './automaticRoadData';
import {decodeOpenFreeMapRoadTile,OFM_EVIDENCE_RULE,OFM_METADATA,type OpenFreeMapEvidenceRequest,type OpenFreeMapTileSnapshot} from './openFreeMapRoadData';
import {buildTileRoadPath} from './ofmDerivedTilePath';
import {createReferenceGraphRepository,type GraphPoint} from './referenceGraph';
import {eligibleRoadGraph} from './roadFollowing';
import {fitRoadFollow} from './roadFollowFit';
import {ROAD_TILE_BOUNDARY_LIMIT_HINT,type RoadDrawingPick,type RoadDrawingResult} from './roadDrawingEngine';
import type {useAutomaticRoad} from './useAutomaticRoad';

type RoadInputs=Pick<ReturnType<typeof useAutomaticRoad>,'observeRouteTiles'|'readTileEvidenceBatch'>;
type Cell=readonly [number,number];
export type AcceptedTramRoadSnap={tile:OpenFreeMapEvidenceRequest;pick:RoadDrawingPick};

// The worker emits a source occurrence only for confirmed authored-road
// selection. Bind it to the EXACT tile snapshot used by that selection.
export function bindTramRoadSnap(snapshot:RoadSnapshot|undefined,pick:RoadDrawingPick|undefined):AcceptedTramRoadSnap|undefined{
 if(!pick)return;
 try{return {tile:observedTramRoadAnchor(snapshot,pick).anchor.tile,pick};}catch{return undefined;}
}
const STABLE_POINT_M=.01;
const END_SNAP_M=20;

function cellAt(p:GraphPoint):Cell|null{
 if(!Number.isFinite(p.longitude)||!Number.isFinite(p.latitude)||Math.abs(p.longitude)>=180||Math.abs(p.latitude)>=85)return null;
 return [Math.floor((p.longitude+180)/360*16384),Math.floor((1-Math.asinh(Math.tan(p.latitude*Math.PI/180))/Math.PI)/2*16384)];
}
const sameCell=(a:Cell|null,b:Cell)=>!!a&&a[0]===b[0]&&a[1]===b[1];
const id=(r:string,c:Cell)=>`${OFM_METADATA}/${r}/14/${c[0]}/${c[1]}.pbf`;
function completeTile(t:RoadSnapshot,revision:string,c:Cell):t is OpenFreeMapTileSnapshot{
 const tile=t as Partial<OpenFreeMapTileSnapshot>;
 return Array.isArray(tile.tileLines)&&t.bundle.graph.dataset.revision===revision&&t.bundle.source.source===id(revision,c)
   &&/^[a-f0-9]{64}$/.test(t.bundle.sourceSha256);
}
const pickFrom=async(graph:ReturnType<typeof eligibleRoadGraph>,p:GraphPoint,cell:Cell,radiusM:number):Promise<RoadDrawingPick|null>=>{
 const found=await createReferenceGraphRepository(graph).nearest({point:p,mode:'road',radiusM,limit:8});
 const best=found.candidates[0];if(!best||found.truncated||!sameCell(cellAt(best.point),cell))return null;
 // Adjacent segments at the exact same vertex are one source occurrence;
 // separate fragments or two occurrences remain ambiguous and are refused.
 const position=best.edge.segmentIndex+best.fraction;
 if(found.candidates.some(c=>c.distanceM<=best.distanceM+STABLE_POINT_M&&
     (c.edge.osmWayId!==best.edge.osmWayId||Math.abs(c.edge.segmentIndex+c.fraction-position)>1e-6)))return null;
 return {fragmentId:best.edge.osmWayId,segmentIndex:best.edge.segmentIndex,fraction:best.fraction,point:best.point};
};
const refuse=()=>Error('道路のつながりを確認できません。区間を短くして選び直してください。');

export async function reacquireTramRoadSeam(
 snapshot:RoadSnapshot|undefined,start:GraphPoint,proof:AcceptedTramRoadSnap,end:GraphPoint,endMustMatch:boolean,provider:RoadInputs,
 current:()=>boolean,signal:AbortSignal,
):Promise<Extract<RoadDrawingResult,{kind:'route'}>|null>{
 const live=()=>!signal.aborted&&current();
 const {tile:bound,pick:from}=proof;
 const previous:Cell=[bound.x,bound.y],next=cellAt(end);
 if(!live()||!snapshot?.tileSnapshots||snapshot.tileSnapshots.length>16||!sameCell(cellAt(start),previous)||!next||
    Math.abs(previous[0]-next[0])+Math.abs(previous[1]-next[1])!==1)return null;
 const {revision,source, id:datasetId}=snapshot.bundle.graph.dataset;
 if(!/^[A-Za-z0-9_]{1,64}$/.test(revision)||source!==OFM_METADATA||datasetId!==`ofm-tiles/${revision}`||bound.revision!==revision||bound.z!==14||bound.decoderRule!==OFM_EVIDENCE_RULE||
    !/^[a-f0-9]{64}$/.test(bound.rawSha256)||distanceM(from.point,start)>STABLE_POINT_M)return null;
 // Do not retry a known disconnect. The previous cell must really be absent
 // from the observed viewport, not merely missing an eligible road.
 if(snapshot.tileSnapshots.some(t=>t.bundle.source.source===id(revision,previous)))return null;
 const visible=snapshot.tileSnapshots.filter(t=>completeTile(t,revision,next));
 if(visible.length!==1)return null;
 const existing=visible[0] as OpenFreeMapTileSnapshot;
 const nextGraph=eligibleRoadGraph(existing.bundle);
 const to=await pickFrom(nextGraph,end,next,endMustMatch?STABLE_POINT_M:END_SNAP_M);
 if(!live()||!to)return null;
 const [rawRef,...extra]=await provider.observeRouteTiles([previous],revision,signal);
 if(!live())return null;
 if(extra.length||!rawRef||rawRef.x!==previous[0]||rawRef.y!==previous[1]||rawRef.z!==14||rawRef.revision!==revision||
   rawRef.decoderRule!==OFM_EVIDENCE_RULE||rawRef.rawSha256!==bound.rawSha256)throw Error('以前の道路データが変わりました。起点を置き直してください。');
 const ref:OpenFreeMapEvidenceRequest={...rawRef};
 const evidence=await provider.readTileEvidenceBatch([ref],signal);
 if(!live()||evidence.status==='stale')return null;
 if(evidence.status!=='ready')throw Error(evidence.reason);
 const {bindings,rawTiles}=evidence.evidence;
 const b=bindings[0],raw=rawTiles[0];
 if(bindings.length!==1||rawTiles.length!==1||!b||!raw||b.key!==id(revision,previous)||b.url!==b.key||b.x!==ref.x||b.y!==ref.y||
   b.z!==14||b.revision!==revision||b.decoderRule!==OFM_EVIDENCE_RULE||b.rawSha256!==ref.rawSha256||raw.sha256!==ref.rawSha256)throw refuse();
 const old=await decodeOpenFreeMapRoadTile(raw.bytes,ref.x,ref.y,revision,0);
 if(!live())return null;
 if(old.bundle.sourceSha256!==ref.rawSha256)throw refuse();
 const oldGraph=eligibleRoadGraph(old.bundle);
 const fragment=old.tileLines.find(line=>line.id===from.fragmentId);
 const edge=oldGraph.edges.find(e=>e.osmWayId===from.fragmentId&&e.segmentIndex===from.segmentIndex);
 const a=fragment?.coordinates[from.segmentIndex],z=fragment?.coordinates[from.segmentIndex+1];
 if(!fragment||!edge||!a||!z||!Number.isFinite(from.fraction)||from.fraction<0||from.fraction>1)
  throw refuse();
 const matched={longitude:a[0]+(z[0]-a[0])*from.fraction,latitude:a[1]+(z[1]-a[1])*from.fraction};
 if(!sameCell(cellAt(matched),previous)||distanceM(matched,start)>STABLE_POINT_M||distanceM(matched,from.point)>STABLE_POINT_M)
  throw Error('以前の起点を道路上で確認できません。起点を置き直してください。');
 const allowed=new Set([...oldGraph.edges,...nextGraph.edges].map(edge=>edge.osmWayId));
 const lines=[...old.tileLines,...existing.tileLines].filter(line=>allowed.has(line.id))
   .map(({oneway:_motorDirection,...line})=>line);
 let route:ReturnType<typeof buildTileRoadPath>;
 try{route=buildTileRoadPath(lines,from,to);}catch(error){
  if(error instanceof Error&&error.message==='join-segment-limit')throw Error(ROAD_TILE_BOUNDARY_LIMIT_HINT);
  throw error;
 }
 if(!live())return null;
 if(route.orderedSpans.length!==2||route.joins.length!==1)throw refuse();
 const geometry=route.points.map(([longitude,latitude])=>({longitude,latitude}));
 geometry[0]=start;
 if(endMustMatch)geometry[geometry.length-1]=end;
 return {kind:'route',point:endMustMatch?end:to.point,line:fitRoadFollow(geometry),roadSnap:to};
}
