import {TrackStroke} from './TrackStroke';
import {useEffect,useMemo,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {MAP_VIEWBOX,project,type Camera} from './map';
import {firstByKey} from './mapLookupIndex';
import {trackBounds,trackViewport,intersectsTrackView} from './trackViewport';
import type {GeoPoint} from './alignment';
import type {SectionInfrastructure} from './sectionInfrastructure';
type Endpoint={alignment_id:string;endpoint:'start'|'end'};
export type TrackRenderGeometry={internal_movements?:{points:GeoPoint[]}[];lanes:{alignment_id:string;section_index:number;lane_index:number;points:GeoPoint[]}[];movements:{movement:{from:{endpoint:Endpoint;lane_index:number};to:{endpoint:Endpoint;lane_index:number};point:unknown};from_position:number;to_position:number;points:GeoPoint[]}[]};
export function useTrackRenderGeometry(world:unknown,enabled:boolean){
 const [state,setState]=useState<{world:unknown;value:TrackRenderGeometry|null;error:string|null}|null>(null);
 useEffect(()=>{
  if(!enabled||!world)return;let alive=true;
  void invoke<TrackRenderGeometry>('preview_track_render_geometry',{world}).then(value=>{
   if(!Array.isArray(value?.lanes)||!Array.isArray(value?.movements))throw Error('描画データが不正です');
   if(alive)setState({world,value,error:null});
  }).catch(error=>{if(alive)setState({world,value:null,error:String(error)});});
  return()=>{alive=false;};
 },[world,enabled]);
 return enabled&&state&&state.world===world?state:{world,value:null,error:null};
}
export function TrackGeometryLayer({geometry,camera,lines,selectedId,mapSize=MAP_VIEWBOX}:{geometry:TrackRenderGeometry;camera:Camera;lines:{id:string;infrastructure?:SectionInfrastructure[]}[];selectedId:string;mapSize?:{width:number;height:number}}){
 const byId=useMemo(()=>firstByKey(lines,line=>line.id),[lines]);
 const indexed=useMemo(()=>{
  const pairRepresentatives=<T extends {points:GeoPoint[]}>(items:readonly T[])=>items.filter((_,i)=>i%2===0).map((item,index)=>({item,index,bounds:trackBounds(item.points)}));
  return {
   lanes:geometry.lanes.map((item,index)=>({item,index,bounds:trackBounds(item.points)})),
   movements:pairRepresentatives(geometry.movements),
   internal:pairRepresentatives(geometry.internal_movements??[]),
  };
 },[geometry]);
 const bounds=trackViewport(camera,mapSize);
 const path=(points:GeoPoint[])=>points.map((point,index)=>{const p=project(point.latitude,point.longitude,camera);return `${index?'L':'M'}${p.x},${p.y}`;}).join(' ');
 return <g className="physical-track-geometry" pointerEvents="none">{indexed.lanes.map(({item:lane,index,bounds:area})=>{
  if(!intersectsTrackView(area,bounds))return null;
  const infrastructure=byId.get(lane.alignment_id)?.infrastructure?.[lane.section_index];
  return <TrackStroke key={index} data-physical-alignment={lane.alignment_id} data-lane-index={lane.lane_index} d={path(lane.points)} className={selectedId===lane.alignment_id?'alignment-path alignment-selected':'alignment-path'} style={{strokeDasharray:infrastructure?.level.startsWith('underground')?'5 4':undefined,opacity:1}}/>;
 })}{indexed.movements.map(({item,index,bounds:area})=>intersectsTrackView(area,bounds)?<TrackStroke key={'movement'+index} data-track-movement={index} d={path(item.points)} className="alignment-path turnout-lane"/>:null)}
 {indexed.internal.map(({item,index,bounds:area})=>intersectsTrackView(area,bounds)?<TrackStroke key={'internal'+index} data-internal-track-movement={index} d={path(item.points)} className="alignment-path turnout-lane"/>:null)}</g>;
}
