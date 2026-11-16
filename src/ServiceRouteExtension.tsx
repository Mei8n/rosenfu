import {useEffect,useRef,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {extensionAvailable,extensionStops} from './routeExtensionModel';
import {resolveServicePath,referenceKey,sameSegment,sameOccurrence} from './servicePaths';
import type {ServicePath,PathWorld,PathPreview,StopReference} from './servicePaths';
import type {Segment} from './RouteCandidatePreview';
import type {CreateServicePath} from './ServicePathCreator';
import {ServiceStopDiagram} from './ServiceStopDiagram';

type Props={service:{id:string;name:string;start:string};path:ServicePath;route:{id:string;name:string;start:string;segments:Segment[]};world:PathWorld;stations:{id:string;name:string}[];enabled:boolean;onDirty:(dirty:boolean)=>void;onSave:(draft:CreateServicePath)=>Promise<void>};
export function ServiceRouteExtension({service,path,route,world,stations,enabled,onDirty,onSave}:Props){
 const [preview,setPreview]=useState<PathPreview|null>(null),[stops,setStops]=useState<StopReference[]>([]),[busy,setBusy]=useState(false),[error,setError]=useState(''),[stale,setStale]=useState(false);
 const epoch=useRef(0),pending=useRef(false),previousWorld=useRef(world);
 useEffect(()=>{if(previousWorld.current!==world){epoch.current++;if(preview||pending.current){setStale(true);setError('編集中にワールドが更新されました。入力は保持しています。取り消して最新の経路を確認してください。');}setBusy(false);pending.current=false;previousWorld.current=world;}return()=>{epoch.current++;};},[world]);
 useEffect(()=>()=>onDirty(false),[onDirty]);
 if(!extensionAvailable(service.start,path,route)&&!preview&&!stale)return null;
 const names=new Map(stations.map(s=>[s.id,s.name]));
 async function inspect(){
  if(!enabled||stale||pending.current)return;pending.current=true;onDirty(true);setBusy(true);setError('');const token=++epoch.current;
  try{const result=await invoke<PathPreview>('preview_service_path_replacement',{serviceId:service.id,start:service.start,segments:route.segments});if(token!==epoch.current)return;
   const expected=resolveServicePath(service.start,result.legs,world);
   if(result.legs.length!==route.segments.length||result.legs.some((leg,i)=>leg.key!==String(BigInt(path.next_leg_key)+BigInt(i))||!sameSegment(leg.segment,route.segments[i]))||expected.length!==result.occurrences.length||expected.some((o,i)=>!sameOccurrence(o,result.occurrences[i])))throw Error('延伸経路を確認できませんでした');
   setStops(extensionStops(service.start,path,result,world));setPreview(result);onDirty(true);
  }catch(e){if(token===epoch.current){setError(String(e));onDirty(false);}}finally{if(token===epoch.current){pending.current=false;setBusy(false);}}
 }
 async function save(){if(!enabled||stale||!preview||pending.current)return;pending.current=true;setBusy(true);setError('');try{await onSave({name:service.name,start:service.start,sourceRouteId:route.id,segments:route.segments,stops});setPreview(null);onDirty(false);}catch(e){setError(String(e));}finally{pending.current=false;setBusy(false);}}
 return <section aria-label="路線に合わせて系統を延伸"><h4>路線の延伸を系統に追加</h4><p>{route.name}の終点まで延伸できます。既存の停車・通過設定を維持し、追加駅は停車にします。</p>
 {!preview?<button disabled={!enabled||busy||stale} onClick={()=>void inspect()}>追加する駅を確認</button>:<><ServiceStopDiagram label="延伸後の停車駅" controlLabel={i=>`延伸後の停車駅 ${i+1}`} disabled={!enabled||busy||stale} rows={preview.occurrences.map((o,i)=>({fixed:i===0||i===preview.occurrences.length-1,key:referenceKey(o.reference),name:names.get(o.station_id)??'駅',stopping:stops.some(s=>referenceKey(s)===referenceKey(o.reference))}))} onChange={(key,stopping)=>{const ref=preview.occurrences.find(o=>referenceKey(o.reference)===key)!.reference;setStops(old=>stopping?[...old,ref]:old.filter(s=>referenceKey(s)!==referenceKey(ref)));}}/><button disabled={!enabled||busy||stale} onClick={()=>void save()}>この系統を延伸</button><button className="secondary" disabled={busy} onClick={()=>{epoch.current++;setPreview(null);setStale(false);setError('');onDirty(false);}}>キャンセル</button></>}
 {!preview&&stale&&<button className="secondary" onClick={()=>{setStale(false);setError('');onDirty(false);}}>キャンセル</button>}
 {error&&<p role="alert">{error}</p>}
 </section>;
}
