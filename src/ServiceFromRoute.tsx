import {useEffect,useRef,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {resolveServicePath,sameOccurrence,sameSegment,referenceKey} from './servicePaths';
import type {PathWorld,PathPreview} from './servicePaths';
import type {Segment} from './RouteCandidatePreview';
import type {CreateServicePath} from './ServicePathCreator';
import {ServiceStopDiagram} from './ServiceStopDiagram';
export function ServiceFromRoute({routes,paths,stations,world,enabled,onDirty,onSave}:{routes:{id:string;name:string;start:string}[];paths:{route_id:string;segments:Segment[]}[];stations:{id:string;name:string}[];world:PathWorld;enabled:boolean;onDirty:(v:boolean)=>void;onSave:(v:CreateServicePath)=>Promise<void>}){
 const [routeId,setRouteId]=useState(''),[name,setName]=useState(''),[preview,setPreview]=useState<PathPreview|null>(null),[error,setError]=useState(''),[busy,setBusy]=useState(false);
 const epoch=useRef(0);const names=new Map(stations.map(s=>[s.id,s.name]));
 useEffect(()=>{epoch.current++;setPreview(null);setBusy(false);return()=>{epoch.current++;};},[world,enabled]);
 async function select(id:string){
  const token=++epoch.current;setRouteId(id);setPreview(null);setError('');onDirty(!!id);
  const route=routes.find(r=>r.id===id),path=paths.find(p=>p.route_id===id);if(!route||!path)return;
  setName(route.name);setBusy(true);
  try{const result=await invoke<PathPreview>('preview_service_path',{start:route.start,segments:path.segments});if(token!==epoch.current)return;
   if(!Array.isArray(result.legs)||result.legs.length!==path.segments.length||result.legs.some((leg,i)=>leg.key!==String(i+1)||!sameSegment(leg.segment,path.segments[i])))throw Error('経路を確認できませんでした。');
   const expected=resolveServicePath(route.start,result.legs,world);
   if(!Array.isArray(result.occurrences)||expected.length!==result.occurrences.length||expected.some((o,i)=>!sameOccurrence(o,result.occurrences[i])))throw Error('停車駅を確認できませんでした。');setPreview(result);
  }catch(e){if(token===epoch.current)setError(String(e));}finally{if(token===epoch.current)setBusy(false);}
 }
 async function save(){const route=routes.find(r=>r.id===routeId),path=paths.find(p=>p.route_id===routeId);if(!enabled||busy||!route||!path||!preview||!name.trim())return;setBusy(true);try{await onSave({name:name.trim(),sourceRouteId:route.id,start:route.start,segments:path.segments,stops:preview.occurrences.map(o=>o.reference)});setRouteId('');setPreview(null);onDirty(false);}catch(e){setError(String(e));}finally{setBusy(false);}}
 return <section aria-label="既存路線から系統を作成"><h4>路線から作成</h4><fieldset disabled={!enabled||busy}><label>路線<select aria-label="系統にする路線" value={routeId} onChange={e=>void select(e.target.value)}><option value="">路線を選択</option>{routes.map(route=><option key={route.id} value={route.id}>{route.name}</option>)}</select></label>{routeId&&<label>系統名<input autoComplete="off" value={name} maxLength={80} onChange={e=>setName(e.target.value)}/></label>}{preview&&<ServiceStopDiagram label="停車駅" controlLabel={i=>`停車駅 ${i+1}`} onChange={()=>{}} disabled rows={preview.occurrences.map(o=>({key:referenceKey(o.reference),name:names.get(o.station_id)??'駅',stopping:true}))}/>}<button disabled={!preview||!name.trim()} onClick={()=>void save()}>系統を作成</button></fieldset><button className="secondary" disabled={!enabled} onClick={()=>{epoch.current++;setBusy(false);setRouteId('');setPreview(null);setError('');onDirty(false);}}>キャンセル</button>{busy&&<p role="status">経路を確認中…</p>}{error&&<p role="alert">{error}</p>}</section>;
}
