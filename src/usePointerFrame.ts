import {useCallback,useEffect,useRef} from 'react';

/** Keep the latest visual task per kind, without retaining React events.
 * Committing input (click/up/cancel) must cancel this queue and process its own
 * exact coordinates synchronously. No edit or history action is queued here. */
export function usePointerFrame(){
 const tasks=useRef(new Map<string,()=>void>());
 const frame=useRef<number|null>(null);
 const cancel=useCallback(()=>{
  if(frame.current!==null)cancelAnimationFrame(frame.current);
  frame.current=null;tasks.current.clear();
 },[]);
 const schedule=useCallback((kind:string,task:()=>void)=>{
  if(typeof requestAnimationFrame!=='function'){task();return;}
  tasks.current.set(kind,task);
  if(frame.current!==null)return;
  frame.current=requestAnimationFrame(()=>{
   frame.current=null;
   const pending=[...tasks.current.values()];tasks.current.clear();
   for(const run of pending)run();
  });
 },[]);
 useEffect(()=>cancel,[cancel]);
 return {schedule,cancel};
}
