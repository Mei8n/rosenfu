import {useEffect,useRef} from "react";
import type {MutableRefObject} from "react";
import {MAX_ZOOM,MIN_ZOOM,zoomCameraAt} from "./map";
import type {Camera} from "./map";
type Anchor={x:number;y:number};
/** Accumulate high-resolution wheel input; one rAF writer owns the displayed camera. */
export function useSmoothZoom(camera:MutableRefObject<Camera>,onFrame:(camera:Camera)=>void){
 const writer=useRef(onFrame);writer.current=onFrame;
 const frame=useRef<number|null>(null),target=useRef<number|null>(null);
 function cancel(){if(frame.current!==null)cancelAnimationFrame(frame.current);frame.current=null;target.current=null;}
 function zoomBy(delta:number,anchor:Anchor){
  if(!Number.isFinite(delta)||delta===0)return;
  const from=camera.current;
  const goal=Math.max(MIN_ZOOM,Math.min(MAX_ZOOM,(target.current??from.zoom)+delta));
  if(frame.current!==null)cancelAnimationFrame(frame.current);
  target.current=goal;
  if(Math.abs(goal-from.zoom)<1e-8){cancel();return;}
  if(window.matchMedia?.("(prefers-reduced-motion: reduce)").matches){writer.current(zoomCameraAt(from,goal,anchor));cancel();return;}
  // Use only rAF timestamps: embedding/test environments may expose a different
  // performance.now() origin from the window that owns requestAnimationFrame.
  let started:number|null=null;
  function tick(now:number){
   started ??= now;
   const t=Math.max(0,Math.min(1,(now-started)/160));
   const eased=1-(1-t)**3;
   writer.current(zoomCameraAt(from,from.zoom+(goal-from.zoom)*eased,anchor));
   if(t<1)frame.current=requestAnimationFrame(tick);else{frame.current=null;target.current=null;}
  }
  frame.current=requestAnimationFrame(tick);
 }
 useEffect(()=>cancel,[]);
 return {zoomBy,cancel};
}
/** DOM deltaMode: pixels, lines or pages. Cap a single gesture, not accumulated input. */
export function wheelZoomDelta(delta:number,mode:number,pageHeight:number){
 if(!Number.isFinite(delta))return 0;
 const pixels=delta*(mode===1?16:mode===2?pageHeight:1);
 return Math.max(-.75,Math.min(.75,-pixels/400));
}
