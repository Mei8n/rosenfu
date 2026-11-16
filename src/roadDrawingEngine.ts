import {buildTileRoadPath} from './ofmDerivedTilePath';
import type {TileRoadLine} from './ofmBoundaryContinuations';
import {createReferenceGraphRepository,type GraphPoint} from './referenceGraph';
import type {ReferenceGraphBundle} from './referenceGraphBundle';
import {eligibleRoadGraph} from './roadFollowing';
import {createRoadFollowRouter} from './roadFollowRoute';
import {fitRoadFollow} from './roadFollowFit';
export type RoadDrawingQuery={point:GraphPoint;start?:GraphPoint;radiusM:number};
export type RoadDrawingResult={kind:'free'}|{kind:'point';point:GraphPoint}|{kind:'route';point:GraphPoint;line:GraphPoint[]}|{kind:'error';error:string};
export function createRoadDrawingEngine(bundle:ReferenceGraphBundle,tileLines:readonly TileRoadLine[]=[]){
 const graph=eligibleRoadGraph(bundle),nodes=new Map(graph.nodes.map(n=>[n.osmId,n])),repository=createReferenceGraphRepository(graph),route=createRoadFollowRouter(bundle);
 const allowedWays=new Set(graph.edges.map(edge=>edge.osmWayId)),allowedTiles=tileLines.filter(line=>allowedWays.has(line.id));
 const pick=(candidate:Awaited<ReturnType<typeof repository.nearest>>['candidates'][number])=>({datasetId:graph.dataset.id,revision:graph.dataset.revision,edgeId:candidate.edge.id,fraction:candidate.fraction});
 const at=(candidate:Awaited<ReturnType<typeof repository.nearest>>['candidates'][number])=>{const a=nodes.get(candidate.edge.from)!,b=nodes.get(candidate.edge.to)!;return {longitude:a.longitude+(b.longitude-a.longitude)*candidate.fraction,latitude:a.latitude+(b.latitude-a.latitude)*candidate.fraction};};
 return async(query:RoadDrawingQuery):Promise<RoadDrawingResult>=>{
  try{
   const end=(await repository.nearest({point:query.point,mode:'road',radiusM:query.radiusM,limit:1})).candidates[0];
   if(!end)return {kind:'free'};const point=at(end);if(!query.start)return {kind:'point',point};
   const start=(await repository.nearest({point:query.start,mode:'road',radiusM:query.radiusM,limit:1})).candidates[0];
   if(!start)return {kind:'free'};
   let path;
   try{path=route(pick(start),pick(end));}catch(error){
    if(!tileLines.length||start.edge.osmWayId===end.edge.osmWayId)throw error;
    const derived=buildTileRoadPath(allowedTiles,{fragmentId:start.edge.osmWayId,segmentIndex:start.edge.segmentIndex,fraction:start.fraction},{fragmentId:end.edge.osmWayId,segmentIndex:end.edge.segmentIndex,fraction:end.fraction});
    path={points:derived.points.map(([longitude,latitude])=>({longitude,latitude})),lengthM:Infinity};
   }
   if(path.lengthM<.1)return {kind:'point',point};
   // Existing rail endpoints and manually placed anchors remain exact. The short
   // lead onto the nearby road is drawn explicitly, never changes a built rail.
   path.points[0]=query.start;return {kind:'route',point,line:fitRoadFollow(path.points)};
  }catch(error){const message=error instanceof Error?error.message:String(error);return {kind:'error',error:/^[\x00-\x7f]*$/.test(message)?'道路のつながりを確認できません。区間を短くして選び直してください':message};}
 };
}
