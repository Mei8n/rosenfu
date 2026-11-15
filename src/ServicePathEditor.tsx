import {useEffect,useRef,useState} from 'react';
import {referenceKey,resolveServicePath} from './servicePaths';
import type {PathWorld,ServicePath,StopReference} from './servicePaths';
export function ServicePathEditor({service,path,world,stations,enabled,onDirty,onSave}:{service:{id:string;name:string;start:string};path:ServicePath;world:PathWorld;stations:{id:string;name:string}[];enabled:boolean;onDirty:(dirty:boolean)=>void;onSave:(name:string,stops:StopReference[])=>Promise<void>}){
 const [name,setName]=useState(service.name),[stops,setStops]=useState(path.stops),[error,setError]=useState(''),[pending,setPending]=useState(false),[conflict,setConflict]=useState(false);
 const savedSignature=JSON.stringify([service.id,service.name,path]),observed=useRef(savedSignature);
 const occurrences=(()=>{try{return resolveServicePath(service.start,path.legs,world);}catch{return [];}})(),names=new Map(stations.map(s=>[s.id,s.name]));
 const changed=name!==service.name||JSON.stringify(stops)!==JSON.stringify(path.stops);
 function reset(){setName(service.name);setStops(path.stops);setError('');setConflict(false);observed.current=savedSignature;onDirty(false);}
 useEffect(()=>{if(observed.current===savedSignature)return;if(changed&&!pending){setConflict(true);setError('保存済み系統が変更されています。入力は保持しています。取り消して最新の内容を確認してください。');}else reset();},[savedSignature]);
 useEffect(()=>{onDirty(changed);},[changed,onDirty]);
 async function save(){if(!enabled||pending||conflict||!changed||!name.trim()||!occurrences.length)return;setPending(true);setError('');try{await onSave(name.trim(),stops);onDirty(false);}catch(e){setError(String(e));}finally{setPending(false);}}
 return <section aria-label="保存済み新形式系統"><h4>新形式系統: {service.name}</h4><p>系統名と停車・通過を後から変更できます。経路の差し替えは未対応です。駅・路線・線路や系統のIDは変更しません。</p><fieldset disabled={!enabled||pending}><label>保存済み系統名<input value={name} maxLength={80} onChange={e=>setName(e.target.value)}/></label><ol>{occurrences.map((o,i)=>{const key=referenceKey(o.reference),end=i===0||i===occurrences.length-1;return <li key={key}><label><input type="checkbox" aria-label={`保存済み停車 ${i+1}`} checked={stops.some(s=>referenceKey(s)===key)} disabled={end} onChange={e=>setStops(occurrences.filter(x=>referenceKey(x.reference)===key?e.target.checked:stops.some(s=>referenceKey(s)===referenceKey(x.reference))).map(x=>x.reference))}/>{names.get(o.station_id)??o.station_id} / {stops.some(s=>referenceKey(s)===key)?'停車':'通過'}{end?'（端点・停車固定）':''}</label></li>;})}</ol><button type="button" disabled={conflict||!changed||!name.trim()||!occurrences.length} onClick={()=>void save()}>保存済み系統の変更を適用</button><button type="button" disabled={!changed} onClick={reset}>保存済み系統の変更を取り消す</button></fieldset>{error&&<p role="alert">{error}</p>}</section>;
}
