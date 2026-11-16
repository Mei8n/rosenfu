import {resolveServicePath, referenceKey, sameSegment} from './servicePaths';
import type {PathWorld, ServicePath, PathPreview, StopReference} from './servicePaths';
import type {Segment} from './RouteCandidatePreview';

/** Only append to an unchanged physical prefix. Never infer binding or reroute an express. */
export function extensionAvailable(start:string, path:ServicePath, route:{start:string;segments:Segment[]}) {
 return start===route.start && path.legs.length>0 && route.segments.length>path.legs.length &&
  path.legs.every((leg,index)=>sameSegment(leg.segment,route.segments[index]));
}
export function extensionStops(start:string,path:ServicePath,preview:PathPreview,world:PathWorld):StopReference[]{
 const before=resolveServicePath(start,path.legs,world);
 const after=resolveServicePath(start,preview.legs,world);
 if(after.length<=before.length||before.some((o,i)=>o.station_id!==after[i].station_id))throw Error('既存の停車駅の順序が変わるため延伸できません');
 const stops=new Set(path.stops.map(referenceKey));
 return after.filter((_,i)=>i>=before.length||stops.has(referenceKey(before[i].reference))).map(o=>o.reference);
}
