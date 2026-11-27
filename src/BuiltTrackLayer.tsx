import {memo,useMemo,type MouseEvent,type PointerEvent} from 'react';
import type {GeoPoint} from './alignment';
import type {World} from './App';
import {project,type Camera} from './map';
import {displayInfrastructureSections,schematicTrackPaths} from './sectionInfrastructure';
import {TrackStroke} from './TrackStroke';
import {TrackGeometryLayer,type TrackRenderGeometry} from './TrackRenderGeometry';

type Props={
  paths:{alignment:NonNullable<World["alignments"]>[number];hit:string}[];
  mapSize:{width:number;height:number};
  alignments:NonNullable<World['alignments']>;
  camera:Camera;
  geometry:TrackRenderGeometry|null;
  selectedId:string;
  showTitles:boolean;
  onHover:(event:PointerEvent<SVGPathElement>,id:string)=>void;
  onLeave:()=>void;
  onSelect:(event:MouseEvent<SVGPathElement>,id:string)=>void;
};
/** Committed geometry has no dependency on construction cursor state. */
export const BuiltTrackLayer=memo(function BuiltTrackLayer({paths,alignments,camera,mapSize,geometry,selectedId,showTitles,onHover,onLeave,onSelect}:Props){
  const cache=useMemo(()=>new WeakMap<NonNullable<World['alignments']>[number],ReturnType<typeof displayInfrastructureSections>>(),[]);
  const sections=useMemo(()=>paths.map(({alignment,hit})=>{
    let cached=cache.get(alignment);
    if(!cached){cached=displayInfrastructureSections(alignment);cache.set(alignment,cached);}
    return {alignment,hit,sections:cached};
  }),[paths,cache]);
  const drawPath=(points:GeoPoint[])=>points.map((point,index)=>{const p=project(point.latitude,point.longitude,camera);return `${index?'L':'M'}${p.x},${p.y}`;}).join(' ');
  return <>
    {sections.map(({alignment,sections,hit}) => <g key={alignment.id} className="alignment route-base" data-alignment-id={alignment.id}>
      {showTitles&&<title>{alignment.name}</title>}

      {sections.map((section,index)=><g key={index} data-track-count={section.infrastructure.track_count} data-structure-level={section.infrastructure.level} pointerEvents="none">
        {section.infrastructure.level.startsWith('elevated')&&<path d={drawPath(section.points)} fill="none" stroke="#8095a7" strokeWidth={section.infrastructure.track_count*3+6} opacity=".4"/>}
        {!geometry&&schematicTrackPaths(section.points.map(point=>project(point.latitude,point.longitude,camera)),section.infrastructure.track_count).map((path,rail)=><TrackStroke key={rail} d={path} className={selectedId===alignment.id?"alignment-path alignment-selected":"alignment-path"} style={{strokeDasharray:section.infrastructure.level.startsWith('underground')?'5 4':undefined,opacity:1}}/>)}
      </g>)}
      <path d={hit} className="alignment-hit" onPointerEnter={event=>onHover(event,alignment.id)} onPointerMove={event=>onHover(event,alignment.id)} onPointerLeave={onLeave} onClick={event=>onSelect(event,alignment.id)} />
    </g>)}
    {geometry&&<TrackGeometryLayer geometry={geometry} camera={camera} mapSize={mapSize} lines={alignments} selectedId={selectedId}/>}
  </>;
});
