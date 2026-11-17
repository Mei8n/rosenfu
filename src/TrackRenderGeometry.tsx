import {timeConstructionIPC} from './constructionTiming';
import {TrackStroke} from './TrackStroke';
import {memo,useEffect,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {project,type Camera} from './map';
import type {GeoPoint} from './alignment';
import type {SectionInfrastructure} from './sectionInfrastructure';
type Endpoint={alignment_id:string;endpoint:'start'|'end'};
export type TrackRenderGeometry={internal_movements?:{points:GeoPoint[]}[];lanes:{alignment_id:string;section_index:number;lane_index:number;points:GeoPoint[]}[];movements:{movement:{from:{endpoint:Endpoint;lane_index:number};to:{endpoint:Endpoint;lane_index:number};point:unknown};from_position:number;to_position:number;points:GeoPoint[]}[]};
export function useTrackRenderGeometry(world:unknown,enabled:boolean){
 const [state,setState]=useState<{world:unknown;value:TrackRenderGeometry|null;error:string|null}|null>(null);
 useEffect(()=>{
  if(!enabled||!world)return;let alive=true;
  void timeConstructionIPC('preview_track_render_geometry',()=>invoke<TrackRenderGeometry>('preview_track_render_geometry',{world})).then(value=>{
   if(!Array.isArray(value?.lanes)||!Array.isArray(value?.movements))throw Error('描画データが不正です');
   if(alive)setState({world,value,error:null});
  }).catch(error=>{if(alive)setState({world,value:null,error:String(error)});});
  return()=>{alive=false;};
 },[world,enabled]);
 return enabled&&state&&state.world===world?state:{world,value:null,error:null};
}
export const TrackGeometryLayer=memo(function TrackGeometryLayer({geometry,camera,lines,selectedId}:{geometry:TrackRenderGeometry;camera:Camera;lines:{id:string;infrastructure?:SectionInfrastructure[]}[];selectedId:string}){
 const path=(points:GeoPoint[])=>points.map((point,index)=>{const p=project(point.latitude,point.longitude,camera);return `${index?'L':'M'}${p.x},${p.y}`;}).join(' ');
 return <g className="physical-track-geometry" pointerEvents="none">{geometry.lanes.map((lane,index)=>{
  const infrastructure=lines.find(line=>line.id===lane.alignment_id)?.infrastructure?.[lane.section_index];
  return <TrackStroke key={index} data-physical-alignment={lane.alignment_id} data-lane-index={lane.lane_index} d={path(lane.points)} className={selectedId===lane.alignment_id?'alignment-path alignment-selected':'alignment-path'} style={{strokeDasharray:infrastructure?.level.startsWith('underground')?'5 4':undefined,opacity:1}}/>;
 })}{geometry.movements.filter((_,index)=>index%2===0).map((movement,index)=><TrackStroke key={'movement'+index} data-track-movement={index} d={path(movement.points)} className="alignment-path turnout-lane"/>)}{(geometry.internal_movements??[]).filter((_,index)=>index%2===0).map((movement,index)=><TrackStroke key={"internal"+index} data-internal-track-movement={index} d={path(movement.points)} className="alignment-path turnout-lane"/>)}</g>;
});
