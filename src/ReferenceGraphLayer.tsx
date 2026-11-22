import {memo,useMemo} from 'react';
import type {DisplayEdge} from './referenceGraphDisplay';
import {project} from './map';
import type {Camera} from './map';
/** No hit targets: game construction and panning receive the original input. */
export const ReferenceGraphLayer=memo(function ReferenceGraphLayer({edges,camera,tramRoadGuideIds}:{edges:DisplayEdge[];camera:Camera;tramRoadGuideIds?:ReadonlySet<string>}){
 const paths=useMemo(()=>{
  const rail:string[]=[],road:string[]=[],tramGuide:string[]=[];
  for(const item of edges){
   const a=project(item.from.latitude,item.from.longitude,camera),b=project(item.to.latitude,item.to.longitude,camera);
   (item.edge.mode==='rail'?rail:tramRoadGuideIds?.has(item.edge.id)?tramGuide:road).push(`M${a.x},${a.y}L${b.x},${b.y}`);
  }
  return {rail:rail.join(' '),road:road.join(' '),tramGuide:tramGuide.join(' ')};
 },[edges,camera,tramRoadGuideIds]);
 return <g className="reference-graph-layer" pointerEvents="none" aria-hidden="true">
  <path d={paths.road} className="reference-graph-road" fill="none"/>
  {paths.tramGuide&&<path d={paths.tramGuide} className="reference-graph-road tram-road-guide-overlay" fill="none"/>}
  <path d={paths.rail} className="reference-graph-rail" fill="none"/>
 </g>;
});
