import {useEffect,useMemo,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
/** A preview is tied to both the full request and current world, never a previous plan. */
export function useConstructionPreview(world:unknown,request:{alignments:unknown[];stations:unknown[];roads?:unknown[]}|null,waterRevision=0){
 // Roads may contain a full tile: serialize once per snapshot, not per cursor frame.
 const roads=request?.roads;
 const roadKey=useMemo(()=>JSON.stringify(roads??[]),[roads]);
 const key=request?`{"alignments":${JSON.stringify(request.alignments)},"stations":${JSON.stringify(request.stations)},"roads":${roadKey}}`:null;
 const [state,setState]=useState<{world:unknown;key:string;waterRevision:number;error:string|null;value:unknown}|null>(null);
 useEffect(()=>{
  if(!key)return;let alive=true;
  const timer=setTimeout(()=>{void invoke('preview_construction_plan',JSON.parse(key)).then(value=>{if(alive)setState({world,key,waterRevision,error:null,value});}).catch(error=>{if(alive)setState({world,key,waterRevision,error:String(error),value:null});});},150);
  return()=>{alive=false;clearTimeout(timer);};
 },[world,key,waterRevision]);
 return state&&state.world===world&&state.key===key&&state.waterRevision===waterRevision?state:null;
}
