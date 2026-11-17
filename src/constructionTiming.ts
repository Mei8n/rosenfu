import type {ProfilerOnRenderCallback} from 'react';
const prefix='rosenfu.construction';
function record(operation:()=>void){try{operation();}catch{/* Optional diagnostics must never change construction outcomes. */}}
let serial=0;
let active:ConstructionTrace|undefined;
const recent:ConstructionTrace[]=[];
export function constructionTimingEnabled(){
 try{return localStorage.getItem('rosenfu.construction-timing')==='1'&&typeof performance.measure==='function';}catch{return false;}
}
/** Opt-in tracing only. No World serialization or timing-based behavior changes. */
export class ConstructionTrace {
 readonly id=++serial;
 readonly measures:string[]=[];
 worldScheduled=false;
 private sequence=0;
 private completed=false;
 private readonly finishTotal:()=>void;
 constructor(){this.finishTotal=this.span('click-to-first-frame');}
 span(phase:string){
  const name=`${prefix}.${this.id}.${phase}.${++this.sequence}`;
  record(()=>{performance.mark(name);});
  return ()=>record(()=>{performance.measure(name,{start:name,detail:{attempt:this.id,phase}});performance.clearMarks(name);this.measures.push(name);});
 }
 complete(){if(this.completed)return;this.completed=true;this.finishTotal();if(active===this)active=undefined;}
}
export function startConstructionTrace(){
 if(!constructionTimingEnabled())return;
 const trace=new ConstructionTrace();active=trace;recent.push(trace);
 if(recent.length>10)for(const name of recent.shift()!.measures)record(()=>performance.clearMeasures(name));
 return trace;
}
export async function timeConstructionIPC<T>(command:string,operation:()=>Promise<T>):Promise<T>{
 const end=active?.span(`ipc.${command}`);
 try{return await operation();}finally{end?.();}
}
export const recordConstructionRender:ProfilerOnRenderCallback=(_id,phase,actualDuration,_baseDuration,startTime,commitTime)=>{
 if(!active)return;
 const name=`${prefix}.${active.id}.react.render.${active.measures.length}`;
 const trace=active;
 record(()=>{performance.measure(name,{start:startTime,duration:actualDuration,detail:{attempt:trace.id,phase:'react.render',reactPhase:phase,commitTime}});trace.measures.push(name);});
};
