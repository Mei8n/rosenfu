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
 const [error,setError]=useState(''),[draft,setDraft]=useState<Draft>(),[choices,setChoices]=useState<StopOccurrenceChoice[]>([]);
 const serial=useRef(0),locked=useRef(false),mutation=useRef(false);
 const state=useRef<Draft|undefined>(undefined),available=useRef(false),choose=useRef<StopOccurrenceChoice[]>([]);
 const current=useRef(input);current.current=input;
 function publish(next:Draft|undefined){state.current=next;setDraft(next);}
 function setOptions(next:StopOccurrenceChoice[]){choose.current=next;setChoices(next);}
 function cancel(){
  if(mutation.current)return;
  serial.current++;locked.current=false;available.current=false;setActive(false);setBusy(false);setError('');publish(undefined);setOptions([]);
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
 async function verify(stops:BusInteriorStop[],save=false){
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
   if(!save){publish({proof,stops:normalized,preview});return;}
   // Cancel only unknown/no-write stages. Once committing, block cancellation
   // and accept the authoritative world returned by the CAS command.
   mutation.current=true;setCommitting(true);
   await current.current.commit(proof);
   mutation.current=false;setCommitting(false);cancel();
  }catch(e){
   if(valid(job,scope,route.id))setError(displayError(e));
  }finally{
   mutation.current=false;setCommitting(false);
   if(serial.current===job){locked.current=false;setBusy(false);}
  }
 }
 function add(option:StopOccurrenceChoice){
  const cur=state.current;
  if(!cur||locked.current||!available.current||cur.preview.stopOccurrenceKeys[option.segment]!==option.occurrenceKey)return;
  if(cur.stops.length>=64){setError('これ以上停留所を追加できません。');return;}
  if(cur.stops.some(st=>st.occurrenceKey===option.occurrenceKey&&st.fractionQ===option.fractionQ)){
   setError('ここにはすでに停留所があります。');return;
  }
  let ordinal=1;while(cur.stops.some(st=>st.name===`停留所 ${ordinal}`))ordinal++;
  const {occurrenceKey,fractionQ}=option;
  void verify([...cur.stops,{id:`s-${crypto.randomUUID()}`,name:`停留所 ${ordinal}`,occurrenceKey,fractionQ}]);
 }
 function pick(at:GeoPoint,camera:Camera,scale:number){
  const cur=state.current;
  if(!cur||locked.current||!available.current||!current.current.enabled)return;
  setError('');
  try{
   const options=pickBusStopOccurrences(cur.preview,at,camera,scale);
   if(options.length===1){add(options[0]);return;}
   setOptions(options);
   if(!options.length)setError('経路上の、駅や交差点から少し離れた位置を選んでください。');
  }catch(e){setOptions([]);setError(displayError(e));}
 }
 function select(index:number){const option=choose.current[index];if(option)add(option);}
 function rename(id:string,name:string){
  const cur=state.current;if(!cur||locked.current)return;
  publish({...cur,stops:cur.stops.map(st=>st.id===id?{...st,name}:st)});
 }
 function remove(id:string){
  const cur=state.current;if(cur&&!locked.current)void verify(cur.stops.filter(st=>st.id!==id));
 }
 function save(){if(state.current&&!choose.current.length)void verify(state.current.stops,true);}
 return {active,busy,committing,error,draft,choices,start,cancel,pick,select,remove,rename,save,
  cancelChoice:()=>setOptions([]),mutating:()=>mutation.current};
}

export function OrdinaryBusStopsPanel({state}:{state:ReturnType<typeof useOrdinaryBusStops>}){
 const {draft,busy,choices,error}=state;
 return <section aria-label="途中停留所の編集">
  <h3>途中停留所</h3>
  <p>{draft?.proof.path.pattern==='closedOneWay'?'経路上をクリックして追加。起終点はそのまま残ります。途中停留所がない場合は中間に自動停車します。':'経路上をクリックして追加。起点と終点の停留所はそのまま残ります。'}</p>
  {draft?.preview.loopFallbackStop&&<p>{draft.preview.loopFallbackStop.name}（自動・追加すると置き換わります）</p>}
  {busy&&<p role="status">停留所を確認中</p>}
  {draft?.stops.length?<ol>{draft.stops.map((st,i)=>{
    const view=draft.preview.interiorStops?.find(item=>item.id===st.id);
    return <li key={st.id}>
     <label>停留所名 {i+1}<input value={st.name} autoComplete="off" maxLength={200} disabled={busy} onChange={e=>state.rename(st.id,e.target.value)}/></label>
     {draft.preview.returnPath&&view&&!view.servesReturn&&<span> 往路のみ</span>}
     <button type="button" disabled={busy} onClick={()=>state.remove(st.id)}>この停留所を外す</button>
    </li>;
   })}</ol>:null}
  {choices.length>1&&<fieldset>
   <legend>この位置を経路が複数回通ります。停車する通過区間を選択</legend>
   {choices.map(({occurrenceKey,segment},i)=><button key={occurrenceKey} type="button" disabled={busy} onClick={()=>state.select(i)}>通過 {i+1}（経路の {segment+1} 区間目）</button>)}
   <button type="button" disabled={busy} onClick={state.cancelChoice}>選び直す</button>
  </fieldset>}
  {error&&<p role="alert">{error}</p>}
  {draft&&<button type="button" disabled={busy||!!choices.length||draft.stops.some(st=>!acceptableBusStopName(st.name))}
    onClick={state.save}>停留所の編集を確定</button>}
  <button type="button" disabled={state.committing} onClick={state.cancel}>停留所の編集を取り消す</button>
 </section>;
}
