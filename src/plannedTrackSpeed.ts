import {nearestOnAlignment, type GeoPoint} from './alignment';
import {provisionalCurveSpeed, type CircularDraft} from './circularAlignment';
/** The hovered sampled segment determines the curve limit, not the whole plan. */
export function plannedTrackSpeed(geometry:CircularDraft, target:GeoPoint, designSpeed:number):number|null {
 if(geometry.error||geometry.points.length<2)return null;
 let index=0,distance=Infinity;
 for(let i=0;i<geometry.points.length-1;i++){
  const d=nearestOnAlignment(geometry.points.slice(i,i+2),target).distanceM;
  if(d<distance){distance=d;index=i;}
 }
 const arcs=geometry.arcs.filter(a=>index>=a.startPointIndex&&index<a.endPointIndex);
 return Math.min(designSpeed,...arcs.map(arc=>provisionalCurveSpeed(arc.radiusM)??0));
}
