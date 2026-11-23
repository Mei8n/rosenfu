import {useEffect,useRef,useState} from 'react';
import type {GeoPoint} from './alignment';
import type {Camera} from './map';
import {acceptableBusStopName,pickBusStopOccurrences,verifyBusStopPlacement,type StopOccurrenceChoice} from './ordinaryBusStops';
import type {BusInteriorStop,OrdinaryBusPath,OrdinaryBusPreviewQuery,OrdinaryBusPreviewReply,OrdinaryBusQuery} from './ordinaryBusDraft';

type Input={
 active:boolean;enabled:boolean;scope:unknown;route?:{id:string;name:string;path:OrdinaryBusPath};
 token:()=>Promise<string>;preview:(args:OrdinaryBusPreviewQuery)=>Promise<OrdinaryBusPreviewReply>;
 commit:(args:OrdinaryBusQuery)=>Promise<unknown>;
};
type Draft={proof:OrdinaryBusQuery;stops:BusInteriorStop[];preview:ReturnType<typeof verifyBusStopPlacement>};
const displayError=(e:unknown)=>e instanceof Error?e.message:typeof e==='string'?e:'停留所を変更できませんでした。';
function routePath(path:OrdinaryBusPath,stops:BusInteriorStop[]):OrdinaryBusPath{
 const {interiorStops:_,...baseline}=path;
 return stops.length?{...baseline,interiorStops:stops}:baseline;
}
export function useOrdinaryBusStops(input:Input){
 const [active,setActive]=useState(false),[busy,setBusy]=useState(false),[committing,setCommitting]=useState(false);
 const [movingStopId,setMovingStopId]=useState<string>();
 const [error,setError]=useState(''),[draft,setDraft]=useState<Draft>(),[choices,setChoices]=useState<StopOccurrenceChoice[]>([]);
 const moving=useRef<string|undefined>(undefined);
 const serial=useRef(0),locked=useRef(false),mutation=useRef(false);
 const state=useRef<Draft|undefined>(undefined),available=useRef(false),choose=useRef<StopOccurrenceChoice[]>([]);
 const current=useRef(input);current.current=input;
 function publish(next:Draft|undefined){state.current=next;setDraft(next);}
 function setOptions(next:StopOccurrenceChoice[]){choose.current=next;setChoices(next);}
 function setMoving(id?:string){moving.current=id;setMovingStopId(id);}
 function cancel(){
  if(mutation.current)return;
  serial.current++;locked.current=false;available.current=false;setActive(false);setBusy(false);setError('');publish(undefined);setOptions([]);setMoving();
 }
 useEffect(()=>{
  cancel();
  return ()=>{serial.current++;};
 },[input.active,input.scope,input.route?.id]);
 // A preview or stop draft may outlive a tool/world change, but cannot write.
 const valid=(job:number,scope:unknown,id:string)=>serial.current===job&&available.current&&current.current.active&&current.current.scope===scope&&current.current.route?.id===id;
 async function start(){
  const {route,enabled,scope}=current.current;
  if(!enabled||!route||locked.current||mutation.current)return;
  cancel();
  const job=++serial.current;available.current=true;locked.current=true;setActive(true);setBusy(true);
  try{
   const token=await current.current.token();if(!valid(job,scope,route.id))return;
   const proof:OrdinaryBusQuery={expectedWorldToken:token,routeId:route.id,name:route.name,path:route.path,rawEvidence:null,tileBinding:null};
   const reply=await current.current.preview({...proof,requestStopTargets:true});if(!valid(job,scope,route.id))return;
   publish({proof,stops:[...(route.path.interiorStops??[])],preview:verifyBusStopPlacement(proof.path,reply)});
  }catch(e){if(valid(job,scope,route.id))setError(displayError(e));}
  finally{if(serial.current===job){locked.current=false;setBusy(false);}}
 }
 async function verify(stops:BusInteriorStop[],save=false,afterPreview?:()=>void){
  const cur=state.current,{scope,route}=current.current;
  if(!route||!cur||locked.current||!available.current||!current.current.enabled)return;
  if(stops.some(st=>!acceptableBusStopName(st.name))){setError('停留所名を確認してください。');return;}
  const normalized=stops.map(st=>({...st,name:st.name.trim()}));
  const job=++serial.current;locked.current=true;setBusy(true);setError('');setOptions([]);
  const proof={...cur.proof,path:routePath(cur.proof.path,normalized)};
  try{
   const reply=await current.current.preview({...proof,requestStopTargets:true});
   if(!valid(job,scope,route.id)||!current.current.enabled)return;
   const preview=verifyBusStopPlacement(proof.path,reply);
   if(!save){publish({proof,stops:normalized,preview});afterPreview?.();return;}
   // Cancel only unknown/no-write stages. Once committing, block cancellation
   // and accept the authoritative world returned by the CAS command.
   mutation.current=true;setCommitting(true);
   await current.current.commit(proof);
   mutation.current=false;setCommitting(false);cancel();
  }catch(e){
   if(valid(job,scope,route.id))setError(displayError(e));
  }finally{
   if(serial.current===job){mutation.current=false;setCommitting(false);locked.current=false;setBusy(false);}
  }
 }
 function startMove(id:string){
  if(locked.current||!available.current||!current.current.enabled||!state.current?.stops.some(st=>st.id===id))return;
  setOptions([]);setError('');setMoving(id);
 }
 function cancelMove(){
  if(!moving.current||mutation.current)return;
  serial.current++;locked.current=false;setBusy(false);setOptions([]);setError('');setMoving();
 }
 function place(option:StopOccurrenceChoice){
  const cur=state.current,id=moving.current;
  if(!cur||locked.current||!available.current||!current.current.enabled||cur.preview.stopOccurrenceKeys[option.segment]!==option.occurrenceKey)return;
  if(!id&&cur.stops.length>=64){setError('これ以上停留所を追加できません。');return;}
  const {occurrenceKey,fractionQ}=option;
  const same=(stop:BusInteriorStop)=>stop.occurrenceKey===occurrenceKey&&stop.fractionQ===fractionQ;
  if(cur.stops.some(st=>st.id!==id&&same(st))){
   setError('ここにはすでに停留所があります。');return;
  }
  if(id){
   const existing=cur.stops.find(st=>st.id===id);if(!existing)return;
   if(same(existing)){cancelMove();return;}
   void verify(cur.stops.map(st=>st.id===id?{...st,occurrenceKey,fractionQ}:st),false,()=>setMoving());
   return;
  }
  let ordinal=1;while(cur.stops.some(st=>st.name===`停留所 ${ordinal}`))ordinal++;
  void verify([...cur.stops,{id:`s-${crypto.randomUUID()}`,name:`停留所 ${ordinal}`,occurrenceKey,fractionQ}]);
 }
 function pick(at:GeoPoint,camera:Camera,scale:number){
  const cur=state.current;
  if(!cur||locked.current||!available.current||!current.current.enabled)return;
  setError('');
  try{
   const options=pickBusStopOccurrences(cur.preview,at,camera,scale);
   if(options.length===1){place(options[0]);return;}
   setOptions(options);
   if(!options.length)setError('経路上の、駅や交差点から少し離れた位置を選んでください。');
  }catch(e){setOptions([]);setError(displayError(e));}
 }
 function select(index:number){const option=choose.current[index];if(option)place(option);}
 function rename(id:string,name:string){
  const cur=state.current;if(!cur||locked.current||moving.current)return;
  publish({...cur,stops:cur.stops.map(st=>st.id===id?{...st,name}:st)});
 }
 function remove(id:string){
  const cur=state.current;if(cur&&!locked.current&&!moving.current)void verify(cur.stops.filter(st=>st.id!==id));
 }
 function save(){if(state.current&&!choose.current.length&&!moving.current)void verify(state.current.stops,true);}
 return {active,busy,committing,error,draft,choices,movingStopId,start,cancel,startMove,cancelMove,pick,select,remove,rename,save,
  cancelChoice:()=>setOptions([]),mutating:()=>mutation.current};
}

export function OrdinaryBusStopsPanel({state}:{state:ReturnType<typeof useOrdinaryBusStops>}){
 const {draft,busy,choices,error,movingStopId}=state;
 return <section aria-label="途中停留所の編集">
  <h3>途中停留所</h3>
  <p>{draft?.proof.path.pattern==='closedOneWay'?'経路上をクリックして追加。起終点はそのまま残ります。途中停留所がない場合は中間に自動停車します。':'経路上をクリックして追加。起点と終点の停留所はそのまま残ります。'}</p>
  {movingStopId&&<p>停留所の移動先を経路上でクリック</p>}
  {movingStopId&&<button type="button" onClick={state.cancelMove}>位置の変更を取り消す</button>}
  {draft?.preview.loopFallbackStop&&<p>{draft.preview.loopFallbackStop.name}（自動・追加すると置き換わります）</p>}
  {busy&&<p role="status">停留所を確認中</p>}
  {draft?.stops.length?<ol>{draft.stops.map((st,i)=>{
    const view=draft.preview.interiorStops?.find(item=>item.id===st.id);
    return <li key={st.id}>
     <label>停留所名 {i+1}<input value={st.name} autoComplete="off" maxLength={200} disabled={busy||!!movingStopId} onChange={e=>state.rename(st.id,e.target.value)}/></label>
     {draft.preview.returnPath&&view&&!view.servesReturn&&<span> 往路のみ</span>}
     <button type="button" disabled={busy||!!movingStopId||!!choices.length} onClick={()=>state.startMove(st.id)}>停留所 {i+1} の位置を変更</button>
     <button type="button" disabled={busy||!!movingStopId} onClick={()=>state.remove(st.id)}>この停留所を外す</button>
    </li>;
   })}</ol>:null}
  {choices.length>1&&<fieldset>
   <legend>この位置を経路が複数回通ります。停車する通過区間を選択</legend>
   {choices.map(({occurrenceKey,segment},i)=><button key={occurrenceKey} type="button" disabled={busy} onClick={()=>state.select(i)}>通過 {i+1}（経路の {segment+1} 区間目）</button>)}
   <button type="button" disabled={busy} onClick={state.cancelChoice}>選び直す</button>
  </fieldset>}
  {error&&<p role="alert">{error}</p>}
  {draft&&<button type="button" disabled={busy||!!choices.length||!!movingStopId||draft.stops.some(st=>!acceptableBusStopName(st.name))}
    onClick={state.save}>停留所の編集を確定</button>}
  <button type="button" disabled={state.committing} onClick={state.cancel}>停留所の編集を取り消す</button>
 </section>;
}
