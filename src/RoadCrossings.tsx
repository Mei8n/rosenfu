import {timeConstructionIPC} from './constructionTiming';
import {useEffect,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import type {RoadAssessments,SavedRoadSnapshot} from './constructionRoads';
/** Results are tied to the exact world object. Edits, load and Undo hide old
 * assets immediately, and late IPC responses cannot restore stale crossings. */
export function useRoadAssessments(world:({road_snapshots?:SavedRoadSnapshot[]})|null,enabled:boolean):{world:unknown;value:RoadAssessments;error:string|null;pending:boolean}{
 const [state,setState]=useState<{world:unknown;value:RoadAssessments;error:string|null}|null>(null);
 useEffect(()=>{
  if(!enabled||!world||!world.road_snapshots?.length)return;
  let alive=true;
  void timeConstructionIPC('road_assessments',()=>invoke<RoadAssessments>('road_assessments')).then(value=>{
   if(!Array.isArray(value)||value.some(item=>!Array.isArray(item)||typeof item[0]!=='string'||!Array.isArray(item[1]?.crossings)||typeof item[1]?.roads_ready!=='boolean'||typeof item[1]?.unknown_road_height!=='boolean'))throw Error('道路交差の判定結果が不正です');
   if(alive)setState({world,value,error:null});
  }).catch(error=>{if(alive)setState({world,value:[],error:String(error)});});
  return()=>{alive=false;};
 },[world,enabled]);
 return enabled&&state?.world===world?{...state,pending:false}:{world,value:[],error:null,pending:enabled&&!!world?.road_snapshots?.length};
}
