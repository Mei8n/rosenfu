import {sameConnection} from './connectedCorridor';
import type {EndpointConnection} from './destinationJoin';
import type {Candidates,Segment} from './RouteCandidatePreview';
import {referenceKey,resolveServicePath} from './servicePaths';
import type {PathWorld,ServicePath} from './servicePaths';

export type RefugePlanningWorld = PathWorld & {service_paths?: ServicePath[]};
const touches = (link:EndpointConnection,ids:Set<string>)=>ids.has(link.a.alignment_id)||ids.has(link.b.alignment_id);
const branchPath = (segments:Segment[])=>{
 if(segments.some(segment=>segment.kind!=='connected_alignments'))return null;
 const links=segments.flatMap(segment=>segment.kind==='connected_alignments'?segment.connections:[]);
 if(links.length!==2)return null;
 const [first,last]=links;
 const ids=[first.a.alignment_id,first.b.alignment_id,last.b.alignment_id];
 if(new Set(ids).size!==3||last.a.alignment_id!==ids[1]||first.b.endpoint===last.a.endpoint)return null;
 return {links,ids};
};

/** Only suggest an unoccupied branch through a complete, exact two-branch
 * candidate set. This is route guidance; the dispatcher still proves geometry,
 * train fit and reservations when the service is configured for operation. */
export function suggestRefugeBypass(result:Candidates,world:RefugePlanningWorld){
 if(result.truncated||result.status!=='ambiguous'||result.candidates.length!==2||!world.service_paths?.length)return null;
 const [a,b]=result.candidates;
 const start=a.stations[0],end=a.stations.at(-1);
 if(!start||!end||start===end||start!==b.stations[0]||end!==b.stations.at(-1))return null;
 const paths=result.candidates.map(candidate=>branchPath(candidate.segments));
 const [p,q]=paths;
 if(!p||!q||p.ids[0]!==q.ids[0]||p.ids[2]!==q.ids[2]||p.ids[1]===q.ids[1]
   ||p.links[0].a.endpoint!==q.links[0].a.endpoint||p.links[1].b.endpoint!==q.links[1].b.endpoint)return null;
 const component=new Set([...p.ids,...q.ids]);
 const incident=world.alignment_connections?.filter(link=>touches(link,component));
 if(incident?.length!==4)return null;
 const sameLinks=(a:EndpointConnection[],b:EndpointConnection[])=>a.length===b.length&&a.every((link,i)=>sameConnection(link,b[i]));
 // Any other existing path into this facility makes the planning context
 // ambiguous. This also excludes an existing service on the proposed branch.
 const active=world.service_paths.filter(path=>path.legs.some(({segment})=>segment.kind==='alignment'
  ?component.has(segment.alignment_id):segment.connections.some(link=>touches(link,component))));
 if(active.length!==1)return null;
 const [local]=active;
 const lp=branchPath(local.legs.map(leg=>leg.segment));
 if(!lp)return null;
 const occupied=sameLinks(lp.links,p.links)?0:sameLinks(lp.links,q.links)?1:-1;
 if(occupied<0)return null;
 const index=1-occupied;
 try{
  const calls=resolveServicePath(start,local.legs,world).filter(entry=>local.stops.some(stop=>referenceKey(stop)===referenceKey(entry.reference)));
  const localStops=calls.map(call=>call.station_id);
  const parked=world.station_placements?.filter(row=>row.station_id===localStops[1]);
  if(localStops.length!==3||localStops[0]!==start||localStops[2]!==end||parked?.length!==1||parked[0].alignment_id!==lp.ids[1])return null;
  const bypass=resolveServicePath(start,result.candidates[index].segments.map((segment,i)=>({key:String(i+1),segment})),world);
  if(bypass.length!==2||bypass[1].station_id!==end)return null;
  return {index,stopId:localStops[1]};
 }catch{return null;}
}
