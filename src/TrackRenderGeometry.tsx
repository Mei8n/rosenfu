import {TrackStroke} from './TrackStroke';
import {useEffect,useMemo,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {MAP_VIEWBOX,mapViewport,project,viewBounds,type Camera} from './map';
import {firstByKey} from './mapLookupIndex';
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
type Bounds=ReturnType<typeof viewBounds>;
function boundsOf(points:GeoPoint[]):Bounds{
 let west=Infinity,east=-Infinity,north=-Infinity,south=Infinity;
 for(const p of points){
  west=Math.min(west,p.longitude);east=Math.max(east,p.longitude);
  south=Math.min(south,p.latitude);north=Math.max(north,p.latitude);
 }
 return {west,east,north,south};
}
/** Cosmetic only: use the actual native lane/movement points, never the centreline.
 * A long segment with both endpoints outside still intersects its bounding box. */
const intersects=(bounds:Bounds,view:Bounds)=>bounds.west<=view.east&&bounds.east>=view.west&&bounds.north>=view.south&&bounds.south<=view.north;
export function TrackGeometryLayer({geometry,camera,lines,selectedId,mapSize=MAP_VIEWBOX}:{geometry:TrackRenderGeometry;camera:Camera;lines:{id:string;infrastructure?:SectionInfrastructure[]}[];selectedId:string;mapSize?:{width:number;height:number}}){
 const byId=useMemo(()=>firstByKey(lines,line=>line.id),[lines]);
 const indexed=useMemo(()=>{
  const pairRepresentatives=<T extends {points:GeoPoint[]}>(items:readonly T[])=>items.filter((_,i)=>i%2===0).map((item,index)=>({item,index,bounds:boundsOf(item.points)}));
  return {
   lanes:geometry.lanes.map((item,index)=>({item,index,bounds:boundsOf(item.points)})),
   movements:pairRepresentatives(geometry.movements),
   internal:pairRepresentatives(geometry.internal_movements??[]),
  };
 },[geometry]);
 const bounds=viewBounds(camera,mapSize,64+16/mapViewport(mapSize.width,mapSize.height).scale);
 const path=(points:GeoPoint[])=>points.map((point,index)=>{const p=project(point.latitude,point.longitude,camera);return `${index?'L':'M'}${p.x},${p.y}`;}).join(' ');
 return <g className="physical-track-geometry" pointerEvents="none">{indexed.lanes.map(({item:lane,index,bounds:area})=>{
  if(!intersects(area,bounds))return null;
  const infrastructure=byId.get(lane.alignment_id)?.infrastructure?.[lane.section_index];
  return <TrackStroke key={index} data-physical-alignment={lane.alignment_id} data-lane-index={lane.lane_index} d={path(lane.points)} className={selectedId===lane.alignment_id?'alignment-path alignment-selected':'alignment-path'} style={{strokeDasharray:infrastructure?.level.startsWith('underground')?'5 4':undefined,opacity:1}}/>;
 })}{indexed.movements.map(({item,index,bounds:area})=>intersects(area,bounds)?<TrackStroke key={'movement'+index} data-track-movement={index} d={path(item.points)} className="alignment-path turnout-lane"/>:null)}
 {indexed.internal.map(({item,index,bounds:area})=>intersects(area,bounds)?<TrackStroke key={'internal'+index} data-internal-track-movement={index} d={path(item.points)} className="alignment-path turnout-lane"/>:null)}</g>;
}
