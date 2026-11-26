import {memo} from 'react';
import {project,type Camera} from './map';
import type {GeoPoint} from './alignment';
import type {World} from './App';
import {routeAppearance} from './routeAppearance';
import {serviceBandPaths,serviceRenderChains} from './serviceMapAppearance';

type Props={
  paths:{alignment:NonNullable<World['alignments']>[number]}[];
  camera:Camera;
  routeVisual:ReturnType<typeof routeAppearance>;
  serviceMapSections:(ReturnType<typeof serviceRenderChains>[number]&{key:string})[];
  casing:number;
  weight:number;
};
export const RouteLayer=memo(function RouteLayer({paths,camera,routeVisual,serviceMapSections,casing,weight}:Props){
  const drawPath=(points:GeoPoint[])=>points.map((point,index)=>{const p=project(point.latitude,point.longitude,camera);return `${index?'L':'M'}${p.x},${p.y}`;}).join(' ');
  return <>
    <g className="route-assignment-overlays" pointerEvents="none">{paths.flatMap(({alignment})=>routeVisual.alignmentSections.get(alignment.id)?.map((points,index)=><path key={`${alignment.id}-${index}`} d={drawPath(points)} className="alignment-path route-assigned-section" />)??[])}</g>
    {serviceMapSections.map(section=><g key={section.key} className="service-map-section" data-alignment-id={section.alignmentIds[0]} data-service-count={section.bands.length} pointerEvents="none">
      <title>{section.bands.map(b=>b.name).join(" / ")}</title>
      <path d={drawPath(section.points)} fill="none" stroke="white" strokeWidth={casing} strokeLinecap="butt" strokeLinejoin="round"/>
      {serviceBandPaths(section.points.map(p=>project(p.latitude,p.longitude,camera)),section.bands,weight).map(band=><path key={band.serviceId} data-service-id={band.serviceId} d={band.d} fill="none" stroke={band.color} strokeWidth={band.width} strokeLinecap="round" strokeLinejoin="round"/>)}
    </g>)}
  </>;
});
