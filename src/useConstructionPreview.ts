import {useEffect,useMemo,useRef,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
type Scope={world:unknown;waterRevision:number};
type Job={scope:Scope;key:string;settle:(error:string|null,value:unknown)=>void};
/** One native validation at a time; replace queued cursor positions with the latest.
 * Results still belong to the exact request and world that produced them. */
export function useConstructionPreview(world:unknown,request:{alignments:unknown[];stations:unknown[];roads?:unknown[]}|null,waterRevision=0){
 const roads=request?.roads;
 const roadKey=useMemo(()=>JSON.stringify(roads??[]),[roads]);
 const key=request?`{"alignments":${JSON.stringify(request.alignments)},"stations":${JSON.stringify(request.stations)},"roads":${roadKey}}`:null;
 const [state,setState]=useState<{scope:Scope;key:string;error:string|null;value:unknown}|null>(null);
 const scope=useRef<Scope|null>(null);
 const running=useRef<Job|null>(null),queued=useRef<Job|null>(null);
 useEffect(()=>{
  if(!key){scope.current=null;return;}
  if(!scope.current||scope.current.world!==world||scope.current.waterRevision!==waterRevision){
   scope.current={world,waterRevision};
  }
  let alive=true;
  const basis=scope.current;
  const job:Job={scope:basis,key,settle:(error,value)=>{
   if(alive)setState({scope:basis,key,error,value});
  }};
  async function drain(){
   if(running.current)return;
   while(queued.current){
    const next=queued.current;queued.current=null;running.current=next;
    try{
     const value=await invoke('preview_construction_plan',JSON.parse(next.key));
     next.settle(null,value);
    }
    catch(error){next.settle(String(error),null);}
    finally{running.current=null;}
   }
  }
  if(running.current?.scope===job.scope&&running.current.key===key){
   // A → B → A can reuse the still-running A, but never a settled result
   // or a promised result from a retired world/road/water basis.
   running.current.settle=job.settle;
  }else{
   queued.current=job;
   void drain();
  }
  return()=>{alive=false;if(queued.current===job)queued.current=null;};
 },[world,key,waterRevision]);
 return state&&state.scope===scope.current&&state.scope.world===world&&state.key===key&&state.scope.waterRevision===waterRevision?state:null;
}
