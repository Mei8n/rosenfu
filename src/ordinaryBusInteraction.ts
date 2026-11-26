import type {BusAnchor} from './ordinaryBusDraft';

export type DraftControl={id:string;anchor:BusAnchor;};
export type BusControlEnd='start'|'end';
type End=BusControlEnd;
type Body={controls:readonly DraftControl[];active:End;closed?:true};
type Frame<T>={body:Body;verified?:T};
type Check={id:number;body:Body;finish:boolean};
type Refusal={reason:string;leg?:number;attempt:Body};
export type OrdinaryBusInteraction<T>={
 scope:unknown;frame:Frame<T>;past:readonly Frame<T>[];future:readonly Frame<T>[];
 generation:number;limit:number;check?:Check;refusal?:Refusal;
 /** An explicit finish intent bound to a validated exact frame. The reducer never persists. */
 finish?:{id:number;body:Body;verified:T};
};
const HISTORY_LIMIT=48;
const samePoint=(a:BusAnchor,b:BusAnchor)=>a.tile.x===b.tile.x&&a.tile.y===b.tile.y&&
 a.tile.revision===b.tile.revision&&a.tile.rawSha256===b.tile.rawSha256&&
 ((a.raw.feature===b.raw.feature&&a.raw.line===b.raw.line&&a.raw.vertex===b.raw.vertex)||
  a.point.latitude===b.point.latitude&&a.point.longitude===b.point.longitude);
const bodyCopy=(b:Body):Body=>({...b,controls:[...b.controls]});
const trail=<T>(state:OrdinaryBusInteraction<T>)=>[...state.past.slice(-(HISTORY_LIMIT-1)),state.frame];

export function emptyBusInteraction<T>(scope:unknown,limit=16):OrdinaryBusInteraction<T>{
 if(!Number.isInteger(limit)||limit<2||limit>64)throw Error('invalid bus control limit');
 return {scope,frame:{body:{controls:[],active:'end'}},past:[],future:[],generation:0,limit};
}
function refuse<T>(s:OrdinaryBusInteraction<T>,body:Body,reason:string,leg?:number):OrdinaryBusInteraction<T>{
 return {...s,generation:s.generation+1,check:undefined,finish:undefined,refusal:{reason,leg,attempt:body}};
}
function propose<T>(s:OrdinaryBusInteraction<T>,body:Body):OrdinaryBusInteraction<T>{
 if(body.closed&&(body.controls.length<3||!samePoint(body.controls[0].anchor,body.controls.at(-1)!.anchor)))
  return refuse(s,body,'環状系統には起点と別の経由点が必要です。');
 if(body.controls.length>s.limit)return refuse(s,body,'経由点が多すぎます。');
 // Duplicate stable ID (history/conflict), or empty next segment, is never submitted.
 if(new Set(body.controls.map(c=>c.id)).size!==body.controls.length||
    body.controls.some((c,i)=>i>0&&samePoint(c.anchor,body.controls[i-1].anchor)))
  return refuse(s,body,'同じ道路の点を続けて置けません。');
 const next={...s,generation:s.generation+1,check:undefined,finish:undefined,refusal:undefined};
 if(body.controls.length<2)return {...next,frame:{body:bodyCopy(body)},past:trail(s),future:[]};
 return {...next,check:{id:next.generation,body:bodyCopy(body),finish:false}};
}
export function requestBusFinish<T>(s:OrdinaryBusInteraction<T>):OrdinaryBusInteraction<T>{
 if(s.refusal||s.finish)return s;
 if(s.check)return s.check.finish?s:{...s,check:{...s.check,finish:true}};
 const {body,verified}=s.frame;
 return body.controls.length>=2&&verified!==undefined?{...s,finish:{id:s.generation,body,verified}}:s;
}
/** A primary click may extend; double click requests only the/latest proposal,
 * never adds the last point a second time. Marked handles are app hit targets,
 * not approximated by looking at physical preview polyline coordinates. */
export function clickBusControl<T>(s:OrdinaryBusInteraction<T>,pick:DraftControl|null,detail=1):OrdinaryBusInteraction<T>{
 if(detail>=2)return requestBusFinish(s);
 if(!pick||s.finish)return s;
 const body=s.check?.body??s.frame.body,controls=body.controls;
 const index=controls.findIndex(c=>samePoint(c.anchor,pick.anchor));
 const active=body.active==='start'?0:controls.length-1;
 if(index>=0){
  if(controls.length===1)return s;
  if(index===active)return requestBusFinish(s);
  if(index===0||index===controls.length-1){
   const next=index===0?'start':'end';
   return s.check?{...s,check:{...s.check,finish:false,body:{...body,active:next}}}:
     {...s,frame:{...s.frame,body:{...body,active:next}}};
  }
  return removeBusControl(s,controls[index].id);
 }
 return propose(s,{...body,controls:body.active==='start'?[pick,...controls]:[...controls,pick]});
}
export function closeBusCircuit<T>(s:OrdinaryBusInteraction<T>,startHandleId:string,terminalId:string):OrdinaryBusInteraction<T>{
 const body=s.check?.body??s.frame.body,controls=body.controls;
 if(s.finish||body.closed||body.active!=='end'||controls.length<2||controls[0].id!==startHandleId)return s;
 return propose(s,{...body,closed:true,controls:[...controls,{id:terminalId,anchor:controls[0].anchor}]});
}
export function selectBusEnd<T>(s:OrdinaryBusInteraction<T>,end:End):OrdinaryBusInteraction<T>{
 const body=s.check?.body??s.frame.body;
 if(body.active===end)return s;
 return s.check?{...s,check:{...s.check,finish:false,body:{...body,active:end}}}:
   {...s,frame:{...s.frame,body:{...body,active:end}},finish:undefined};
}
export function moveBusControl<T>(s:OrdinaryBusInteraction<T>,id:string,anchor:BusAnchor):OrdinaryBusInteraction<T>{
 if(s.finish)return s;
 const body=s.check?.body??s.frame.body,prev=body.controls.find(c=>c.id===id);
 if(!prev||samePoint(prev.anchor,anchor))return s;
 const both=body.closed&&(prev===body.controls[0]||prev===body.controls.at(-1));
 return propose(s,{...body,controls:body.controls.map((c,i)=>c.id===id||both&&(i===0||i===body.controls.length-1)?{id:c.id,anchor}:c)});
}
export function insertBusControl<T>(s:OrdinaryBusInteraction<T>,afterId:string,pick:DraftControl):OrdinaryBusInteraction<T>{
 if(s.finish)return s;
 const body=s.check?.body??s.frame.body,idx=body.controls.findIndex(c=>c.id===afterId);
 if(idx<0||idx===body.controls.length-1)return s;
 return propose(s,{...body,controls:[...body.controls.slice(0,idx+1),pick,...body.controls.slice(idx+1)]});
}
export function removeBusControl<T>(s:OrdinaryBusInteraction<T>,id:string):OrdinaryBusInteraction<T>{
 if(s.finish)return s;
 const body=s.check?.body??s.frame.body;
 if(body.closed&&(id===body.controls[0].id||id===body.controls.at(-1)?.id))return s;
 const controls=body.controls.filter(c=>c.id!==id);
 return controls.length===body.controls.length?s:propose(s,{...body,controls});
}
export function settleBusValidation<T>(
 s:OrdinaryBusInteraction<T>,scope:unknown,id:number,outcome:{verified:T;resolvedEnd?:DraftControl}|{reason:string;leg?:number},
):OrdinaryBusInteraction<T>{
 const check=s.check;
 if(scope!==s.scope||!check||check.id!==id)return s;
 if('reason' in outcome){
  const leg=Number.isInteger(outcome.leg)&&outcome.leg!>=0&&outcome.leg!<check.body.controls.length-1?outcome.leg:undefined;
  return refuse(s,check.body,outcome.reason,leg);
 }
 const resolved=outcome.resolvedEnd;
 const controls=check.body.controls,index=resolved?controls.findIndex(c=>c.id===resolved.id):-1;
 if(resolved&&(check.body.closed||index<0||index!==0&&index!==controls.length-1||
    controls.some(c=>c.id!==resolved.id&&samePoint(c.anchor,resolved.anchor))))
  return refuse(s,check.body,'経路の確認結果が一致しません。位置を選び直してください。');
 const body=resolved?{...check.body,controls:controls.map(c=>c.id===resolved.id?resolved:c)}:check.body;
 const frame={body,verified:outcome.verified};
 return {...s,frame,past:trail(s),future:[],check:undefined,refusal:undefined,
  // A double click while checking must not commit a different landing.
  finish:check.finish&&!resolved?{id,body:frame.body,verified:frame.verified}:undefined};
}
/** Ctrl+Z clears an unvalidated/pending attempt first, otherwise changes
 * only the local draft. It never calls world's Undo. */
export function undoBusDraft<T>(s:OrdinaryBusInteraction<T>):OrdinaryBusInteraction<T>{
 if(s.check||s.refusal||s.finish)return {...s,generation:s.generation+1,check:undefined,refusal:undefined,finish:undefined};
 const prev=s.past.at(-1);
 return prev?{...s,generation:s.generation+1,frame:prev,past:s.past.slice(0,-1),future:[s.frame,...s.future]}:s;
}
export function redoBusDraft<T>(s:OrdinaryBusInteraction<T>):OrdinaryBusInteraction<T>{
 if(s.check||s.refusal||s.finish||!s.future.length)return s;
 return {...s,generation:s.generation+1,frame:s.future[0],past:trail(s),future:s.future.slice(1)};
}
export function cancelBusInteraction<T>(s:OrdinaryBusInteraction<T>):OrdinaryBusInteraction<T>{
 return {...emptyBusInteraction<T>(s.scope,s.limit),generation:s.generation+1};
}
