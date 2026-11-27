import {ServiceStopDiagram} from './ServiceStopDiagram';
import {useEffect,useMemo,useRef,useState} from 'react';
import {referenceKey,resolveServicePath} from './servicePaths';
import type {PathWorld,ServicePath,StopReference} from './servicePaths';
export function ServicePathEditor({service,path,world,stations,enabled,onDirty,onSave,onSaved}:{service:{id:string;name:string;start:string};path:ServicePath;world:PathWorld;stations:{id:string;name:string}[];enabled:boolean;onDirty:(dirty:boolean)=>void;onSave:(name:string,stops:StopReference[])=>Promise<void>;onSaved?:()=>void}){
 const [name,setName]=useState(service.name),[stops,setStops]=useState(path.stops),[error,setError]=useState(''),[pending,setPending]=useState(false),[conflict,setConflict]=useState(false);
 // A successful save and pending=false can arrive in the same React batch.
 // A snapshot matching the draft is acknowledged, not an external conflict.
 // Report completion after the child has committed pending=false, not when the
 // parent first publishes a new world snapshot. Never notify after unmount.
 const alive=useRef(true),notified=useRef(0);
 const [completed,setCompleted]=useState(0);
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
 useEffect(()=>{if(completed===0||notified.current===completed)return;notified.current=completed;onSaved?.();},[completed,onSaved]);
 const savedSignature=JSON.stringify([service.id,service.name,path]),observed=useRef(savedSignature);
 const rows=useMemo(()=>{
  try {
   const occurrences=resolveServicePath(service.start,path.legs,world);
   const totals=new Map<string,number>(),seen=new Map<string,number>();
   for(const {station_id} of occurrences)totals.set(station_id,(totals.get(station_id)??0)+1);
   return occurrences.map(({station_id,reference},i)=>{
    const visit=(seen.get(station_id)??0)+1;
    seen.set(station_id,visit);
    return {key:referenceKey(reference),stationId:station_id,reference,
     fixed:i===0||i===occurrences.length-1,
     detail:(totals.get(station_id)??0)>1?`${visit}回目`:undefined};
   });
  } catch {return [];}
 },[service.start,path.legs,world]);
 const names=useMemo(()=>new Map(stations.map(s=>[s.id,s.name])),[stations]);
 const stopKeys=useMemo(()=>new Set(stops.map(referenceKey)),[stops]);
 function changeStop(key:string,checked:boolean){
  setStops(previous=>{
   const nextKeys=new Set(previous.map(referenceKey));
   if(checked)nextKeys.add(key);else nextKeys.delete(key);
   return rows.filter(row=>nextKeys.has(row.key)).map(row=>row.reference);
  });
 }
 const changed=name!==service.name||JSON.stringify(stops.map(referenceKey))!==JSON.stringify(path.stops.map(referenceKey));
 function reset(){setName(service.name);setStops(path.stops);setError('');setConflict(false);observed.current=savedSignature;onDirty(false);}
 useEffect(()=>{if(observed.current===savedSignature)return;const previous=JSON.parse(observed.current);const wasDirty=name!==previous[1]||JSON.stringify(stops.map(referenceKey))!==JSON.stringify(previous[2].stops.map(referenceKey));if(wasDirty&&(name.trim()!==service.name||JSON.stringify(stops.map(referenceKey))!==JSON.stringify(path.stops.map(referenceKey)))){setConflict(true);setError('保存済み系統が変更されています。入力は保持しています。取り消して最新の内容を確認してください。');}else reset();},[savedSignature]);
 useEffect(()=>{onDirty(changed);},[changed,onDirty]);
 async function save(){if(!enabled||pending||conflict||!changed||!name.trim()||!rows.length)return;setPending(true);setError('');try{await onSave(name.trim(),stops);if(alive.current){onDirty(false);setCompleted(value=>value+1);}}catch(e){if(alive.current)setError(String(e));}finally{if(alive.current)setPending(false);}}
 return <section aria-label="保存済み新形式系統" className="service-stop-editor"><fieldset disabled={!enabled||pending}><label className="service-name-field">名称<input aria-label="保存済み系統名" value={name} maxLength={80} onChange={e=>setName(e.target.value)}/></label><h4>停車駅</h4>
 <ServiceStopDiagram label="保存済み系統の停車駅" controlLabel={(i,row)=>`保存済み停車 ${i+1}${row.detail?` ${row.name}`:''}`} rows={rows.map(row=>{
  const name=names.get(row.stationId)??row.stationId;
  return {key:row.key,name:row.detail?`${name}（${row.detail}）`:name,stopping:stopKeys.has(row.key),fixed:row.fixed,detail:row.detail};
 })} onChange={changeStop}/>
 <div className="stop-actions"><button type="button" aria-label="保存済み系統の変更を適用" disabled={conflict||!changed||!name.trim()||!rows.length} onClick={()=>void save()}>変更を適用</button><button type="button" className="secondary" aria-label="保存済み系統の変更を取り消す" disabled={!changed} onClick={reset}>取り消す</button></div></fieldset>{error&&<p role="alert">{error}</p>}</section>;
}
