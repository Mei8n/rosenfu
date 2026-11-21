import type {TileRoadLine} from './ofmBoundaryContinuations';
import {useEffect,useRef,useState} from 'react';
import type {ReferenceGraphBundle} from './referenceGraphBundle';
import type {RoadDrawingQuery,RoadDrawingResult} from './roadDrawingEngine';
export function useRoadDrawing(bundle:ReferenceGraphBundle|null,enabled:boolean,tileLines:readonly TileRoadLine[]){
 const [preview,setPreview]=useState<RoadDrawingResult|null>(null);
 const worker=useRef<Worker|null>(null),sequence=useRef(0),hoverId=useRef(0);
 type Task={id:number;query:RoadDrawingQuery;click:boolean;resolve:(result:RoadDrawingResult|null)=>void};
 const inFlight=useRef<Task|null>(null),pending=useRef<Task|null>(null);
 const dispatch=()=>{if(inFlight.current||!pending.current||!worker.current)return;inFlight.current=pending.current;pending.current=null;worker.current.postMessage({kind:'query',id:inFlight.current.id,query:inFlight.current.query});};
 const cancel=()=>{hoverId.current=++sequence.current;inFlight.current?.resolve(null);pending.current?.resolve(null);pending.current=null;setPreview(null);};
 useEffect(()=>{
  if(!enabled||!bundle||typeof Worker==='undefined'){cancel();return;}
  const w=new Worker(new URL('./roadDrawing.worker.ts',import.meta.url),{type:'module'});worker.current=w;w.postMessage({kind:'load',bundle,tileLines});
  w.onmessage=e=>{const task=inFlight.current;if(!task||task.id!==e.data.id)return;inFlight.current=null;task.resolve(e.data.result);if(task.id===hoverId.current)setPreview(e.data.result);dispatch();};
  w.onerror=()=>{inFlight.current?.resolve({kind:'error',error:'道路経路を計算できませんでした'});inFlight.current=null;pending.current?.resolve(null);pending.current=null;};
  return()=>{w.terminate();worker.current=null;inFlight.current?.resolve(null);inFlight.current=null;cancel();};
 },[bundle,enabled,tileLines]);
 const query=(query:RoadDrawingQuery,click=false)=>new Promise<RoadDrawingResult|null>(resolve=>{
  const id=++sequence.current;hoverId.current=id;
  if(!worker.current){resolve({kind:'error',error:'道路データを準備中です'});return;}
  // Never replace a queued click with a pointer move. There is at most one
  // calculation and one newest pointer request, not a growing event backlog.
  if(!click&&(pending.current?.click||inFlight.current?.click)){resolve(null);return;}
  if(click&&(pending.current?.click||inFlight.current?.click)){resolve(null);return;}
  pending.current?.resolve(null);pending.current={id,query,click,resolve};dispatch();
 });
 return {preview,cancel,clickPending:()=>!!(inFlight.current?.click||pending.current?.click),hover:(q:RoadDrawingQuery)=>{void query(q);},click:(q:RoadDrawingQuery)=>query(q,true)};
}
