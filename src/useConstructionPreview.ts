import {useEffect,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
/** A preview is tied to both the full request and current world, never a previous plan. */
export function useConstructionPreview(world:unknown,request:{alignments:unknown[];stations:unknown[]}|null){
 const key=request?JSON.stringify(request):null;
 const [state,setState]=useState<{world:unknown;key:string;error:string|null;value:unknown}|null>(null);
 useEffect(()=>{
  if(!key)return;let alive=true;
  const timer=setTimeout(()=>{void invoke('preview_construction_plan',JSON.parse(key)).then(value=>{if(alive)setState({world,key,error:null,value});}).catch(error=>{if(alive)setState({world,key,error:String(error),value:null});});},150);
  return()=>{alive=false;clearTimeout(timer);};
 },[world,key]);
 return state&&state.world===world&&state.key===key?state:null;
}
