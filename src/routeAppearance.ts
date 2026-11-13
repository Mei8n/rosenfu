import { alignmentLength, distanceM, pointOnAlignment } from "./alignment";
import type { GeoPoint } from "./alignment";
import type { Segment } from "./RouteCandidatePreview";
type AppearanceWorld = {
  routes: {id:string;start:string;tracks:string[]}[];
  tracks: {id:string;from:string;to:string}[];
  stations:{id:string}[];
  alignments?:{id:string;points:GeoPoint[]}[];
  station_placements?:{station_id:string;alignment_id:string;position:number}[];
  route_paths?:{route_id:string;segments:Segment[];member_stations:string[]}[];
};
/** Saved route membership only: a service, selected draft, or mere adjacency is not a route. */
export function routeAppearance(world: AppearanceWorld | null) {
  const stations = new Set<string>(), tracks = new Set<string>(), alignmentSections = new Map<string,GeoPoint[][]>();
  if(!world)return {stations,tracks,alignmentSections};
  const knownStations=new Set(world.stations.map(s=>s.id));
  const placements=new Map(world.station_placements?.map(p=>[p.station_id,p]));
  const alignments=new Map(world.alignments?.map(a=>[a.id,a]));
  const trackById=new Map(world.tracks.map(t=>[t.id,t]));
  function section(alignmentId:string,from:string,to:string){
    const a=alignments.get(alignmentId),p=placements.get(from),q=placements.get(to);
    if(!a||!p||!q||p.alignment_id!==alignmentId||q.alignment_id!==alignmentId)return;
    const lo=Math.min(p.position,q.position),hi=Math.max(p.position,q.position),total=alignmentLength(a.points);
    if(total<=0||lo<0||hi>1||lo===hi)return;
    const points=[pointOnAlignment(a.points,lo)];let distance=0;
    for(let i=1;i<a.points.length;i++){distance+=distanceM(a.points[i-1],a.points[i]);if(distance/total>lo&&distance/total<hi)points.push(a.points[i]);}
    points.push(pointOnAlignment(a.points,hi));
    const sections=alignmentSections.get(alignmentId)??[];sections.push(points);alignmentSections.set(alignmentId,sections);
  }
  function track(id:string){const t=trackById.get(id);if(!t)return;tracks.add(id);const p=placements.get(t.from),q=placements.get(t.to);if(p&&q&&p.alignment_id===q.alignment_id)section(p.alignment_id,t.from,t.to);}
  for(const route of world.routes){
    const path=world.route_paths?.find(p=>p.route_id===route.id);
    if(path){
      for(const id of path.member_stations)if(knownStations.has(id))stations.add(id);
      for(const s of path.segments)if(s.kind==="track")track(s.track_id);else section(s.alignment_id,s.from,s.to);
    }else{
      if(knownStations.has(route.start))stations.add(route.start);
      for(const id of route.tracks){track(id);const t=trackById.get(id);if(t){if(knownStations.has(t.from))stations.add(t.from);if(knownStations.has(t.to))stations.add(t.to);}}
    }
  }
  return {stations,tracks,alignmentSections};
}
