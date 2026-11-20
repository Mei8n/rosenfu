import {useEffect,useRef} from "react";

const directions:Record<string,readonly [number,number]>={
 ArrowLeft:[-1,0],ArrowRight:[1,0],ArrowUp:[0,-1],ArrowDown:[0,1],
};
function ownsArrowKeys(target:EventTarget|null){
 return (target instanceof HTMLElement && target.isContentEditable) || target instanceof Element && !!target.closest('input,textarea,select,button,summary,[contenteditable="true"],[role="tab"],[role="slider"],[role="dialog"]');
}
/** Held keys drive one frame-based camera writer, independent of OS key repeat. */
export function useKeyboardPan(onPan:(dx:number,dy:number)=>void,isBlocked:()=>boolean){
 const pan=useRef(onPan),blocked=useRef(isBlocked);
 pan.current=onPan;blocked.current=isBlocked;
 useEffect(()=>{
  const pressed=new Set<string>();
  let frame:number|null=null,previous:number|null=null;
  const stop=()=>{pressed.clear();if(frame!==null)cancelAnimationFrame(frame);frame=null;previous=null;};
  const unavailable=()=>blocked.current()||document.hidden||!!document.querySelector('[role="dialog"],dialog[open]');
  const tick=(now:number)=>{
   if(unavailable()){stop();return;}
   const elapsed=previous===null?0:Math.min(50,Math.max(0,now-previous));previous=now;
   let dx=0,dy=0;
   for(const key of pressed){dx+=directions[key][0];dy+=directions[key][1];}
   const length=Math.hypot(dx,dy);
   if(length&&elapsed)pan.current(dx/length*elapsed*.5,dy/length*elapsed*.5);
   frame=requestAnimationFrame(tick);
  };
  const down=(event:KeyboardEvent)=>{
   if(event.defaultPrevented||event.ctrlKey||event.metaKey||event.altKey||event.shiftKey||event.isComposing||ownsArrowKeys(event.target)||unavailable()){stop();return;}
   if(!(event.key in directions)){if(event.key==='+'||event.key==='-'||event.code==='NumpadAdd'||event.code==='NumpadSubtract')stop();return;}
   // After an interrupt, wait for a fresh press instead of OS repeat resuming pan.
   if(event.repeat&&!pressed.has(event.key))return;
   event.preventDefault();pressed.add(event.key);
   if(frame===null)frame=requestAnimationFrame(tick);
  };
  const up=(event:KeyboardEvent)=>{pressed.delete(event.key);if(!pressed.size)stop();};
  const focus=()=>{if(ownsArrowKeys(document.activeElement))stop();};
  window.addEventListener('keydown',down);window.addEventListener('keyup',up);window.addEventListener('blur',stop);
  window.addEventListener('pointerdown',stop);window.addEventListener('wheel',stop,{passive:true});document.addEventListener('focusin',focus);document.addEventListener('visibilitychange',stop);
  return ()=>{stop();window.removeEventListener('keydown',down);window.removeEventListener('keyup',up);window.removeEventListener('blur',stop);window.removeEventListener('pointerdown',stop);window.removeEventListener('wheel',stop);document.removeEventListener('focusin',focus);document.removeEventListener('visibilitychange',stop);};
 },[]);
}
