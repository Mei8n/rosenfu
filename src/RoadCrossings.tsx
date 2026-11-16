import {useEffect,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {project,type Camera} from './map';
import type {RoadAssessments,SavedRoadSnapshot} from './constructionRoads';
/** Results are tied to the exact world object. Edits, load and Undo hide old
 * assets immediately, and late IPC responses cannot restore stale crossings. */
export function useRoadAssessments(world:({road_snapshots?:SavedRoadSnapshot[]})|null,enabled:boolean){
 const [state,setState]=useState<{world:unknown;value:RoadAssessments;error:string|null}|null>(null);
 useEffect(()=>{
  if(!enabled||!world||!world.road_snapshots?.length)return;
  let alive=true;
  void invoke<RoadAssessments>('road_assessments').then(value=>{
   if(!Array.isArray(value)||value.some(item=>!Array.isArray(item)||typeof item[0]!=='string'||!Array.isArray(item[1]?.crossings)||typeof item[1]?.roads_ready!=='boolean'||typeof item[1]?.unknown_road_height!=='boolean'))throw Error('道路交差の判定結果が不正です');
   if(alive)setState({world,value,error:null});
  }).catch(error=>{if(alive)setState({world,value:[],error:String(error)});});
  return()=>{alive=false;};
 },[world,enabled]);
 return enabled&&state?.world===world?{...state,pending:false}:{world,value:[] as RoadAssessments,error:null,pending:enabled&&!!world?.road_snapshots?.length};
}
export function RoadCrossings({assessments,camera}:{assessments:RoadAssessments;camera:Camera}){
 return <g className="automatic-road-crossings" pointerEvents="none">{assessments.flatMap(([id,assessment])=>assessment.crossings.map((crossing,index)=>{
  const p=project(crossing.point.latitude,crossing.point.longitude,camera);
  return <g key={`${id}/${index}`} data-crossing-alignment={id} transform={`translate(${p.x} ${p.y})`}><title>踏切</title><circle r="5" fill="#f8cb4e" stroke="#29343a" strokeWidth="1.5"/><path d="M -2 -2 L 2 2 M -2 2 L 2 -2" stroke="#29343a" strokeWidth="1.5"/></g>;
 }))}</g>;
}
