import {useRef,useState,useEffect,useLayoutEffect,useCallback} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {RouteCandidatePreview,type Candidate,type Membership} from './RouteCandidatePreview';
import {resolveServicePath,referenceKey,sameSegment,sameOccurrence,type ServicePath,type PathWorld,type PathPreview} from './servicePaths';
import type {RouteBinding} from './ServiceRouteBinding';
import type {CreateServicePath} from './ServicePathCreator';
type Props={onCandidateChange:(candidate:Candidate|null)=>void;onSelectionChange:(ids:string[])=>void;service:{id:string;name:string;start:string};path:ServicePath;binding?:RouteBinding;world:PathWorld;stations:{id:string;name:string}[];enabled:boolean;candidateLength:(path:Candidate)=>number;onDirty:(dirty:boolean)=>void;onSave:(draft:CreateServicePath)=>Promise<void>;onMapPick:(target:'start'|'end'|'via'|'sequence'|null,select?:(id:string)=>void)=>void;mapPickTarget:'start'|'end'|'via'|'sequence'|null};
export function ServiceMapEditor(props:Props){
 const {service,path,binding,world,stations,enabled,onDirty,onSave,onMapPick,mapPickTarget,candidateLength}=props;
 const [editing,setEditing]=useState(false);
 const [stopOverrides,setStopOverrides]=useState<Record<string,boolean>>({});
 const selectedCandidate=useRef<Candidate|null>(null);
 const candidateChanged=useCallback((candidate:Candidate|null)=>{selectedCandidate.current=candidate;props.onCandidateChange(candidate);},[props.onCandidateChange]);
 const editEpoch=useRef(0);
 // A native preview is not permission to apply after retiring this editor or
 // briefly disabling editing. Include the committed scope, not only world identity.
 useLayoutEffect(()=>()=>{editEpoch.current++;},[world,service.id,path,binding,enabled]);
 useEffect(()=>()=>{onDirty(false);onMapPick(null);},[]);
 let original:ReturnType<typeof resolveServicePath>;
 try{original=resolveServicePath(service.start,path.legs,world);}catch{return <p>保存済み経路を確認できません。</p>;}
 const repeated=new Set(original.map(o=>o.station_id)).size!==original.length;
 async function save(name:string,membership:Membership){
  const before=editEpoch.current,segments=membership.path.segments,start=membership.path.stations[0];
  const preview=await invoke<PathPreview>('preview_service_path_replacement',{serviceId:service.id,start,segments});
  if(editEpoch.current!==before)throw Error('編集中の状態が変わったため、適用しませんでした。入力は保持しています。');
  const expected=resolveServicePath(start,preview.legs,world);
  if(preview.legs.length!==segments.length||preview.legs.some((l,i)=>l.key!==String(BigInt(path.next_leg_key)+BigInt(i))||!sameSegment(l.segment,segments[i]))||preview.occurrences.length!==expected.length||expected.some((o,i)=>!sameOccurrence(o,preview.occurrences[i])))throw Error('経路を確認できませんでした');
  if(new Set(expected.map(o=>o.station_id)).size!==expected.length)throw Error('同じ駅を複数回通る経路の地図編集には未対応です');
  const stops=new Set(path.stops.map(referenceKey));
  // Station IDs are unambiguous only after rejecting repeated visits on both paths.
  const old=new Map(original.map(o=>[o.station_id,stops.has(referenceKey(o.reference))]));
  await onSave({name,start,segments,...(binding?.kind==='route'?{sourceRouteId:binding.route_id}:binding?.kind==='routes'?{sourceRouteIds:binding.route_ids}:{}),stops:expected.filter((o,i)=>i===0||i===expected.length-1||(stopOverrides[o.station_id]??old.get(o.station_id)??true)).map(o=>o.reference)});
  setEditing(false);onDirty(false);onMapPick(null);
 }
 if(!editing)return <><button disabled={!enabled||!binding||repeated} onClick={()=>{setStopOverrides({});setEditing(true);onDirty(true);}}>地図で系統を編集</button>{repeated&&<p>同じ駅を複数回通る系統は区間編集を利用してください。</p>}</>;
 return <RouteCandidatePreview onCandidateChange={candidateChanged} onSelectionChange={props.onSelectionChange} entityLabel="系統" editRoute={{id:service.id,name:service.name,path:{stations:original.map(o=>o.station_id),segments:path.legs.map(l=>l.segment)}}} onStationClick={id=>{const ids=selectedCandidate.current?.stations;if(!ids||id===ids[0]||id===ids.at(-1)||!ids.includes(id))return false;const found=original.find(o=>o.station_id===id);const stopping=stopOverrides[id]??(!found||path.stops.some(s=>referenceKey(s)===referenceKey(found.reference)));setStopOverrides(values=>({...values,[id]:!stopping}));return true;}} isStopping={id=>{if(id in stopOverrides)return stopOverrides[id];const found=original.find(o=>o.station_id===id);return !found||path.stops.some(s=>referenceKey(s)===referenceKey(found.reference));}} candidateLength={candidateLength} stations={stations} worldVersion={world} enabled={enabled} canApply={enabled} onDirty={onDirty} onSave={save} onCancelEdit={()=>{setEditing(false);onDirty(false);onMapPick(null);}} startSequenceKey={1} onMapPick={onMapPick} mapPickTarget={mapPickTarget}/>;
}
