import {memo} from 'react';
import {project,type Camera} from './map';
import type {GeoPoint} from './alignment';
import type {World} from './App';
import {routeAppearance} from './routeAppearance';
import {serviceBandPaths,serviceRenderChains} from './serviceMapAppearance';

type Props={
  alignments:NonNullable<World['alignments']>;
  camera:Camera;
  routeVisual:ReturnType<typeof routeAppearance>;
  serviceMapSections:ReturnType<typeof serviceRenderChains>;
  showServices:boolean;
};
export const RouteLayer=memo(function RouteLayer({alignments,camera,routeVisual,serviceMapSections,showServices}:Props){
  const drawPath=(points:GeoPoint[])=>points.map((point,index)=>{const p=project(point.latitude,point.longitude,camera);return `${index?'L':'M'}${p.x},${p.y}`;}).join(' ');
  return <>
    <g className="route-assignment-overlays" pointerEvents="none">{alignments.flatMap(alignment=>routeVisual.alignmentSections.get(alignment.id)?.map((points,index)=><path key={`${alignment.id}-${index}`} d={drawPath(points)} className="alignment-path route-assigned-section" />)??[])}</g>
    {showServices&&serviceMapSections.map((section,index)=><g key={section.alignmentIds.join(":")+":"+index} className="service-map-section" data-alignment-id={section.alignmentIds[0]} data-service-count={section.bands.length} pointerEvents="none">
      <title>{section.bands.map(b=>b.name).join(" / ")}</title>
      <path d={drawPath(section.points)} fill="none" stroke="white" strokeWidth="10" strokeLinecap="butt" strokeLinejoin="round"/>
      {serviceBandPaths(section.points.map(p=>project(p.latitude,p.longitude,camera)),section.bands).map(band=><path key={band.serviceId} data-service-id={band.serviceId} d={band.d} fill="none" stroke={band.color} strokeWidth={band.width} strokeLinecap="round" strokeLinejoin="round"/>)}
    </g>)}
  </>;
});
