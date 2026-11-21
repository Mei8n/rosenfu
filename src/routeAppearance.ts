import {corridorSpans} from "./connectedCorridor";
import type {EndpointConnection} from "./destinationJoin";
import { alignmentLength, distanceM, pointOnAlignment } from "./alignment";
import type { GeoPoint } from "./alignment";
import type { Segment } from "./RouteCandidatePreview";
type AppearanceWorld = {
  alignment_connections?:EndpointConnection[];
  routes: {id:string;start:string}[];
  stations:{id:string}[];
  alignments?:{id:string;points:GeoPoint[]}[];
  station_placements?:{station_id:string;alignment_id:string;position:number}[];
  route_paths?:{route_id:string;segments:Segment[];member_stations:string[]}[];
};
/** Geometry only, shared by saved routes and non-persistent candidate overlays. */
export function routeSegmentSections(world:AppearanceWorld,segments:Segment[]) {
  const alignmentSections = new Map<string,GeoPoint[][]>();
  const placements=new Map(world.station_placements?.map(p=>[p.station_id,p]));
  const alignments=new Map(world.alignments?.map(a=>[a.id,a]));
  function section(alignmentId:string,from:string,to:string){
    const a=alignments.get(alignmentId),p=placements.get(from),q=placements.get(to);
    if(!a||!p||!q||p.alignment_id!==alignmentId||q.alignment_id!==alignmentId)return;
    range(alignmentId,p.position,q.position);
  }
  function range(alignmentId:string,begin:number,finish:number){
    const a=alignments.get(alignmentId);if(!a)return;
    const lo=Math.min(begin,finish),hi=Math.max(begin,finish),total=alignmentLength(a.points);
    if(total<=0||lo<0||hi>1||lo===hi)return;
    const points=[pointOnAlignment(a.points,lo)];let distance=0;
    for(let i=1;i<a.points.length;i++){distance+=distanceM(a.points[i-1],a.points[i]);if(distance/total>lo&&distance/total<hi)points.push(a.points[i]);}
    points.push(pointOnAlignment(a.points,hi));
    const sections=alignmentSections.get(alignmentId)??[];sections.push(points);alignmentSections.set(alignmentId,sections);
  }
  for(const s of segments)if(s.kind==="alignment")section(s.alignment_id,s.from,s.to);else {try{for(const span of corridorSpans(s,world))range(span.alignmentId,span.begin,span.finish);}catch{/* Invalid/stale metadata never paints unrelated tracks. */}}
  return alignmentSections;
}
/** Saved route membership only: a service, selected draft, or mere adjacency is not a route. */
export function routeAppearance(world: AppearanceWorld | null) {
  const stations = new Set<string>(), alignmentSections = new Map<string,GeoPoint[][]>();
  if(!world)return {stations,alignmentSections};
  const knownStations=new Set(world.stations.map(s=>s.id));
  for(const route of world.routes){
    const path=world.route_paths?.find(p=>p.route_id===route.id);
    if(!path)continue;
    for(const id of path.member_stations)if(knownStations.has(id))stations.add(id);
    for(const [id,sections] of routeSegmentSections(world,path.segments))alignmentSections.set(id,[...(alignmentSections.get(id)??[]),...sections]);
  }
  return {stations,alignmentSections};
}
