import {useEffect,useMemo,useRef,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
/** One native validation at a time; replace queued cursor positions with the latest.
 * Results still belong to the exact request and world that produced them. */
export function useConstructionPreview(world:unknown,request:{alignments:unknown[];stations:unknown[];roads?:unknown[]}|null,waterRevision=0){
 const roads=request?.roads;
 const roadKey=useMemo(()=>JSON.stringify(roads??[]),[roads]);
 const key=request?`{"alignments":${JSON.stringify(request.alignments)},"stations":${JSON.stringify(request.stations)},"roads":${roadKey}}`:null;
 const [state,setState]=useState<{world:unknown;key:string;waterRevision:number;error:string|null;value:unknown}|null>(null);
 const running=useRef(false);
 const queued=useRef<null|(()=>Promise<void>)>(null);
 useEffect(()=>{
  queued.current=null;
  if(!key)return;let alive=true;
  async function drain(){
   if(running.current)return;
   running.current=true;
   try{while(queued.current){const next=queued.current;queued.current=null;await next();}}
   finally{running.current=false;}
  }
  const timer=setTimeout(()=>{
   queued.current=async()=>{
    try{const value=await invoke('preview_construction_plan',JSON.parse(key));if(alive)setState({world,key,waterRevision,error:null,value});}
    catch(error){if(alive)setState({world,key,waterRevision,error:String(error),value:null});}
   };
   void drain();
  },150);
  return()=>{alive=false;queued.current=null;clearTimeout(timer);};
 },[world,key,waterRevision]);
 return state&&state.world===world&&state.key===key&&state.waterRevision===waterRevision?state:null;
}
