import {TrackStroke} from './TrackStroke';
import {useEffect,useMemo,useRef,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {MAP_VIEWBOX,project,type Camera} from './map';
import {firstByKey} from './mapLookupIndex';
import {createTrackViewportIndex,trackViewport} from './trackViewport';
import type {GeoPoint} from './alignment';
import type {SectionInfrastructure} from './sectionInfrastructure';
type Endpoint={alignment_id:string;endpoint:'start'|'end'};
export type TrackRenderGeometry={internal_movements?:{points:GeoPoint[]}[];lanes:{alignment_id:string;section_index:number;lane_index:number;points:GeoPoint[]}[];movements:{movement:{from:{endpoint:Endpoint;lane_index:number};to:{endpoint:Endpoint;lane_index:number};point:unknown};from_position:number;to_position:number;points:GeoPoint[]}[]};
type PreviewQueue={epoch:number;active:boolean;waiting:(()=>void)|null};
export function useTrackRenderGeometry(world:unknown,enabled:boolean,epoch=0){
 const [state,setState]=useState<{world:unknown;epoch:number;value:TrackRenderGeometry|null;error:string|null}|null>(null);
 const queueRef=useRef<PreviewQueue|null>(null);
 useEffect(()=>{
  if(queueRef.current?.epoch!==epoch)queueRef.current={epoch,active:false,waiting:null};
  if(!enabled||!world)return;
  const queue=queueRef.current;
  let alive=true;
  function start(){
   if(!alive)return;
   queue.active=true;
   void (async()=>{
    try{
     const value=await invoke<TrackRenderGeometry>('preview_track_render_geometry',{world});
     if(!Array.isArray(value?.lanes)||!Array.isArray(value?.movements))throw Error('描画データが不正です');
     if(alive)setState({world,epoch,value,error:null});
    }catch(error){
     if(alive)setState({world,epoch,value:null,error:String(error)});
    }finally{
     queue.active=false;
     const next=queue.waiting;
     queue.waiting=null;
     if(queueRef.current===queue)next?.();
    }
   })();
  }
  // Only the latest waiting edit in this world reaches native code.
  // Loading a different world starts fresh rather than waiting on old work.
  if(queue.active)queue.waiting=start;else start();
  return()=>{alive=false;if(queue.waiting===start)queue.waiting=null;};
 },[world,enabled,epoch]);
 return enabled&&state&&state.world===world&&state.epoch===epoch?state:{world,epoch,value:null,error:null};
}
export function TrackGeometryLayer({geometry,camera,lines,selectedId,mapSize=MAP_VIEWBOX}:{geometry:TrackRenderGeometry;camera:Camera;lines:{id:string;infrastructure?:SectionInfrastructure[]}[];selectedId:string;mapSize?:{width:number;height:number}}){
 const byId=useMemo(()=>firstByKey(lines,line=>line.id),[lines]);
 const indexed=useMemo(()=>{
  const index=<T extends {points:GeoPoint[]}>(items:readonly T[])=>createTrackViewportIndex(items.map((item,index)=>({item,index,points:item.points})));
  const pairs=<T,>(items:readonly T[])=>items.filter((_,i)=>i%2===0);
  return {lanes:index(geometry.lanes),movements:index(pairs(geometry.movements)),internal:index(pairs(geometry.internal_movements??[]))};
 },[geometry]);
 const bounds=trackViewport(camera,mapSize);
 const path=(points:GeoPoint[])=>points.map((point,index)=>{const p=project(point.latitude,point.longitude,camera);return `${index?'L':'M'}${p.x},${p.y}`;}).join(' ');
 return <g className="physical-track-geometry" pointerEvents="none">{indexed.lanes.visible(bounds).map(({item:lane,index})=>{
  const infrastructure=byId.get(lane.alignment_id)?.infrastructure?.[lane.section_index];
  return <TrackStroke key={index} data-physical-alignment={lane.alignment_id} data-lane-index={lane.lane_index} d={path(lane.points)} className={selectedId===lane.alignment_id?'alignment-path alignment-selected':'alignment-path'} style={{strokeDasharray:infrastructure?.level.startsWith('underground')?'5 4':undefined,opacity:1}}/>;
 })}{indexed.movements.visible(bounds).map(({item,index})=><TrackStroke key={'movement'+index} data-track-movement={index} d={path(item.points)} className="alignment-path turnout-lane"/>)}
 {indexed.internal.visible(bounds).map(({item,index})=><TrackStroke key={'internal'+index} data-internal-track-movement={index} d={path(item.points)} className="alignment-path turnout-lane"/>)}</g>;
}
