import {buildTileRoadPath,coveredSourceSegments,type TilePathPick} from './ofmDerivedTilePath';
import type {TileRoadLine} from './ofmBoundaryContinuations';
import {createReferenceGraphRepository,type GraphPoint,type GraphProjection} from './referenceGraph';
import type {ReferenceGraphBundle} from './referenceGraphBundle';
import {eligibleRoadGraph} from './roadFollowing';
import {createRoadFollowRouter} from './roadFollowRoute';
import {fitRoadFollow} from './roadFollowFit';
export type RoadDrawingQuery={point:GraphPoint;start?:GraphPoint;radiusM:number;authoredTramCue?:boolean;resolveAuthoredSeam?:boolean};
export type RoadDrawingPick=TilePathPick & {point:GraphPoint};
export type RoadDrawingResult={kind:'junction';from:RoadDrawingPick;to:RoadDrawingPick}|{kind:'seam';point:GraphPoint}|{kind:'free'}|{kind:'point';point:GraphPoint;roadSnap?:RoadDrawingPick}|{kind:'route';point:GraphPoint;line:GraphPoint[];roadSnap?:RoadDrawingPick}|{kind:'error';error:string};
export const ROAD_TILE_BOUNDARY_LIMIT_HINT = 'この道路の境界をまたいで敷設できません。別の道路を選ぶか、境界の手前まで敷設してください。';
const ROAD_SECTION_LIMIT_HINT = '一度に道路に沿って敷設できる区間の上限です。手前の道路上まで敷設し、そこからの延伸を試してください。';
export function createRoadDrawingEngine(bundle:ReferenceGraphBundle,tileLines:readonly TileRoadLine[]=[]){
 const graph=eligibleRoadGraph(bundle),nodes=new Map(graph.nodes.map(n=>[n.osmId,n])),repository=createReferenceGraphRepository(graph),route=createRoadFollowRouter(bundle);
 const allowedWays=new Set(graph.edges.map(edge=>edge.osmWayId)),allowedTiles=tileLines.filter(line=>allowedWays.has(line.id));
 const observed=new Map(allowedTiles.filter(line=>line.revision===graph.dataset.revision).map(line=>[line.id,line]));
 const ofm=graph.dataset.id.startsWith('ofm-tiles/')&&/^https:\/\/tiles\.openfreemap\.org\/planet(?:\/|$)/.test(graph.dataset.source);
 const scopeByTile=new Map<string,TileRoadLine[]>();
 if(ofm){for(const l of allowedTiles){if(l.revision!==graph.dataset.revision)continue;const key=l.x+'/'+l.y;const list=scopeByTile.get(key);if(list)list.push(l);else scopeByTile.set(key,[l]);}}
 let manual:ReturnType<typeof createRoadFollowRouter>|undefined;
 const pick=(candidate:Awaited<ReturnType<typeof repository.nearest>>['candidates'][number])=>({datasetId:graph.dataset.id,revision:graph.dataset.revision,edgeId:candidate.edge.id,fraction:candidate.fraction});
 const at=(candidate:Awaited<ReturnType<typeof repository.nearest>>['candidates'][number])=>{const a=nodes.get(candidate.edge.from)!,b=nodes.get(candidate.edge.to)!;return {longitude:a.longitude+(b.longitude-a.longitude)*candidate.fraction,latitude:a.latitude+(b.latitude-a.latitude)*candidate.fraction};};
 return async(query:RoadDrawingQuery):Promise<RoadDrawingResult>=>{
  try{
   const raw=(c:GraphProjection):RoadDrawingPick=>({fragmentId:c.edge.osmWayId,segmentIndex:c.edge.segmentIndex,fraction:c.fraction,point:at(c)});
   const end=(await repository.nearest({point:query.point,mode:'road',radiusM:query.radiusM,limit:1})).candidates[0];
   if(!end)return {kind:'free'};const point=at(end);if(query.authoredTramCue&&ofm&&!observed.has(end.edge.osmWayId))return {kind:'error',error:'道路の元データを確認できません。'};if(!query.start)return {kind:'point',point,...(ofm&&query.authoredTramCue&&query.resolveAuthoredSeam?{roadSnap:raw(end)}:{})};
   const start=(await repository.nearest({point:query.start,mode:'road',radiusM:query.radiusM,limit:1})).candidates[0];
   if(!start)return {kind:'free'};
   if(query.authoredTramCue){
    if(!ofm){
     const path=(manual??=createRoadFollowRouter(bundle,true))(pick(start),pick(end));
     if(path.lengthM<.1)return {kind:'point',point};
     path.points[0]=query.start;return {kind:'route',point,line:fitRoadFollow(path.points)};
    }
    const l=observed.get(start.edge.osmWayId),other=observed.get(end.edge.osmWayId);
    if(!l||!other)return {kind:'error',error:'道路の元データを確認できません。'};
    if(l.id!==other.id){
     const from=raw(start),to=raw(end);
     if(l.x===other.x&&l.y===other.y)return {kind:'junction',from,to};
     // An existing cached seam remains available using the established
     // bounded raw overlap evidence, never the coordinate graph fallback.
     if(!query.resolveAuthoredSeam)return {kind:'seam',point};
     const left=Math.min(l.x,other.x),right=Math.max(l.x,other.x);
     const top=Math.min(l.y,other.y),bottom=Math.max(l.y,other.y);
     if((right-left+1)*(bottom-top+1)>16)throw Error('区間を短くして道路の点を選び直してください。');
     const geometryOnly:TileRoadLine[]=[];
     for(let x=left;x<=right;x++)for(let y=top;y<=bottom;y++){
      const lines=scopeByTile.get(x+'/'+y);
      if(lines)for(const {oneway:_motorDirection,...line} of lines)geometryOnly.push(line);
     }
     const seam=buildTileRoadPath(geometryOnly,from,to);
     const line=seam.points.map(([longitude,latitude])=>({longitude,latitude}));
     line[0]=query.start;return {kind:'route',point,line:fitRoadFollow(line),roadSnap:to};
    }
    const lo=start.edge.segmentIndex+start.fraction,hi=end.edge.segmentIndex+end.fraction;
    if(coveredSourceSegments(lo,hi)>256)return {kind:'error',error:ROAD_SECTION_LIMIT_HINT};
    const receipt=query.resolveAuthoredSeam?{roadSnap:raw(end)}:{};
    if(lo===hi)return {kind:'point',point,...receipt};
    const line=[query.start];
    const append=(i:number)=>{const [longitude,latitude]=l.coordinates[i];line.push({longitude,latitude});};
    if(lo<hi){for(let i=Math.floor(lo)+1;i<hi;i++)append(i);}else{for(let i=Math.ceil(lo)-1;i>hi;i--)append(i);}
    line.push(point);
    return {kind:'route',point,line:fitRoadFollow(line),...receipt};
   }
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
  }catch(error){const message=error instanceof Error?error.message:String(error);
   if(message==='join-segment-limit')return {kind:'error',error:ROAD_TILE_BOUNDARY_LIMIT_HINT};
   if(message==='segment-limit'||message==='source-segment-limit')return {kind:'error',error:ROAD_SECTION_LIMIT_HINT};
   return {kind:'error',error:/^[\x00-\x7f]*$/.test(message)?'道路のつながりを確認できません。区間を短くして選び直してください':message};}
 };
}
