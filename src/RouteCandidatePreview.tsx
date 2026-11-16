import {editStationOrder} from './mapStationEdit';
import {ServiceStopDiagram} from './ServiceStopDiagram';
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {EndpointConnection} from "./destinationJoin";
export type Segment = { kind: "alignment"; alignment_id: string; from: string; to: string } | {kind:"connected_alignments";from:string;to:string;connections:EndpointConnection[]};
export type Candidate = { stations: string[]; segments: Segment[] };
export type Candidates = { shortest_candidate_index?: number | null; status: "no_path" | "unique" | "ambiguous" | "incomplete"; candidates: Candidate[]; truncated: boolean };
export type Membership = { path: Candidate; member_stations: string[]; excluded_stations: string[] };
type Props = {
  entityLabel?: string;
  onStationClick?: (id:string)=>boolean;
  isStopping?: (id:string)=>boolean;
  candidateLength?: (candidate:Candidate)=>number;
  onCandidateChange?: (candidate:Candidate|null) => void;
  onSelectionChange?: (stationIds: string[]) => void;
  editRoute?: {id:string;name:string;path?:Candidate}; defaultName?:string;
  onCancelEdit?: () => void;
  startSequenceKey?: number;
  mapPickTarget?: "start" | "end" | "via" | "sequence" | null;
  onMapPick?: (target: "start" | "end" | "via" | "sequence" | null, select?: (id: string) => void) => void;
  stations: { id: string; name: string }[];
  worldVersion: unknown; resetKey?: unknown; onDirty?: (dirty: boolean) => void;
  enabled: boolean; canApply: boolean;
  onSave?: (name: string, membership: Membership) => Promise<void>;
};
/** Station clicks define ordered constraints; only explicit Finish persists the route. */
export function RouteCandidatePreview({onStationClick,isStopping,candidateLength,onCandidateChange,onSelectionChange,entityLabel="路線",editRoute,defaultName="路線1",onCancelEdit,startSequenceKey,mapPickTarget,onMapPick,stations,worldVersion,resetKey,onDirty,enabled,canApply,onSave}:Props){
 const initialName=editRoute?.name??defaultName;
 const [name,setName]=useState(initialName);
 const [ordered,setOrdered]=useState<string[]>([]);
 const editingMap=!!editRoute?.path;
 const stationClick=useRef(onStationClick);stationClick.current=onStationClick;
 const editQueue=useRef(Promise.resolve());
 const orderedRef=useRef<string[]>([]);
 useEffect(()=>{onSelectionChange?.(ordered);},[ordered,onSelectionChange]);
 useEffect(()=>()=>{onSelectionChange?.([]);},[onSelectionChange]);
 const [result,setResult]=useState<Candidates|null>(null);
 const [chosen,setChosen]=useState<number|null>(null);
 const [membership,setMembership]=useState<Membership|null>(null);
 useEffect(()=>{onCandidateChange?.(membership?.path??null);},[membership,onCandidateChange]);
 useEffect(()=>()=>{onCandidateChange?.(null);},[onCandidateChange]);
 const [error,setError]=useState("");
 const [loading,setLoading]=useState(false);
 const [saving,setSaving]=useState(false);
 const [stale,setStale]=useState(false);
 const [showChoices,setShowChoices]=useState(false);
 const generation=useRef(0), pendingSave=useRef(false);
 const blocked=useRef(false);blocked.current=!enabled||stale;
 const names=new Map(stations.map(s=>[s.id,s.name]));
 const stationName=(id:string)=>names.get(id)??`駅 ${id}`;
 function invalidate(){generation.current++;setResult(null);setMembership(null);setChosen(null);setLoading(false);setStale(false);setShowChoices(false);setError("");}
 useEffect(()=>{invalidate();orderedRef.current=[];setOrdered([]);setName(initialName);},[resetKey]);
 useEffect(()=>{generation.current++;setLoading(false);setMembership(null);setShowChoices(false);setStale(orderedRef.current.length>1);return()=>{generation.current++;};},[worldVersion]);
 useEffect(()=>{onDirty?.(ordered.length>0||name!==initialName);},[ordered,name,initialName,onDirty]);
 async function validate(candidate:Candidate,token:number){
  const next=await invoke<Membership>("preview_route_membership",{start:candidate.stations[0],segments:candidate.segments,excludedStations:[]});
  if(token===generation.current)setMembership(next);
 }
 async function search(ids:string[]){
  invalidate();if(ids.length<2)return;
  const token=generation.current;setLoading(true);
  try{
   const response=await invoke<Candidates>("preview_route_candidates",{start:ids[0],end:ids.at(-1),...(ids.length>2?{viaStationIds:ids.slice(1,-1)}:{})});
   if(token!==generation.current)return;
   setResult(response);
   if(response.truncated||response.status==="incomplete")return;
   const shortest=response.shortest_candidate_index;
   const index=Number.isInteger(shortest)&&shortest!>=0&&shortest!<response.candidates.length?shortest!:response.status==="unique"&&response.candidates.length===1?0:null;
   if(index!==null){setChosen(index);await validate(response.candidates[index],token);}
   if(token===generation.current&&response.candidates.length>1)setShowChoices(true);
  }catch(cause){if(token===generation.current)setError(String(cause));}
  finally{if(token===generation.current)setLoading(false);}
 }
 function beginSequence(){
  onMapPick?.("sequence",id=>{
   if(pendingSave.current||blocked.current||!names.has(id))return;
   if(editingMap&&stationClick.current?.(id))return;
   if(editingMap){const session=generation.current;editQueue.current=editQueue.current.then(async()=>{
    if(session!==generation.current||pendingSave.current||blocked.current)return;
    setLoading(true);setError("");
    try{
     if(!candidateLength)throw Error('経路の比較に必要な地図情報がありません');
     const choice=await editStationOrder(orderedRef.current,id,ids=>invoke<Candidates>('preview_route_candidates',{start:ids[0],end:ids.at(-1),...(ids.length>2?{viaStationIds:ids.slice(1,-1)}:{})}),candidateLength);
     if(session!==generation.current)return;
     const next=await invoke<Membership>('preview_route_membership',{start:choice.result.candidates[choice.index].stations[0],segments:choice.result.candidates[choice.index].segments,excludedStations:[]});
     if(session!==generation.current)return;
     orderedRef.current=choice.ids;setOrdered(choice.ids);setResult(choice.result);setChosen(choice.index);setMembership(next);setStale(false);setShowChoices(choice.result.candidates.length>1);
    }catch(cause){if(session===generation.current)setError(String(cause));}
    finally{if(session===generation.current)setLoading(false);}
   });return;}
   if(orderedRef.current.includes(id)){setError("同じ駅は重複して指定できません。");return;}
   const next=[...orderedRef.current,id];orderedRef.current=next;setOrdered(next);void search(next);
  });
 }
 function beginEditing(){
  const path=editRoute?.path;if(!path)return;
  if(new Set(path.stations).size!==path.stations.length){setError('同じ駅を複数回通る経路の地図編集には未対応です。区間編集を利用してください');return;}
  invalidate();orderedRef.current=[...path.stations];setOrdered([...path.stations]);setMembership({path,member_stations:path.stations,excluded_stations:[]});beginSequence();
 }
 useEffect(()=>{if(startSequenceKey){if(editingMap)beginEditing();else beginSequence();}},[startSequenceKey]);
 async function selectCandidate(index:number){
  if(!enabled||saving||!result||result.truncated||result.status==="incomplete"||!result.candidates[index])return;
  const token=++generation.current;setChosen(index);setMembership(null);setLoading(true);setError("");
  try{await validate(result.candidates[index],token);}catch(cause){if(token===generation.current)setError(String(cause));}
  finally{if(token===generation.current)setLoading(false);}
 }
 async function finish(){
  if(!enabled||!canApply||!onSave||!membership||loading||stale||pendingSave.current||!name.trim())return;
  pendingSave.current=true;setSaving(true);setError("");
  try{await onSave(name.trim(),membership);onMapPick?.(null);onDirty?.(false);}
  catch(cause){setError(String(cause));}
  finally{pendingSave.current=false;setSaving(false);}
 }
 return <section className="route-candidate-preview" aria-label={editRoute?`${entityLabel}を編集`:`${entityLabel}を作成`}>
  <h4>{name||initialName}</h4>
  <label>{entityLabel}名<input autoComplete="off" value={name} maxLength={80} disabled={!enabled||saving} onChange={e=>setName(e.target.value)}/></label>
  {editingMap&&<p>{entityLabel==='系統'?'地図の中間駅をクリックすると停車・通過を切り替えます。起点・終点をクリックすると短縮し、新しい駅をクリックすると経路へ追加します。':'地図の駅をクリックすると経由指定を追加、選択済みの駅をクリックすると指定を外します。起点・終点も変更できます。経路上の中間駅は引き続き路線に所属します。'}選択駅の順序を保って経路を再探索します。元の経路から変わる場合もあるため、地図で確認して編集終了で確定してください。</p>}
  <ol aria-label="選択した駅">{ordered.map((id,i)=><li key={id}>{stationName(id)}{i===0?"（始点）":i===ordered.length-1?"（終点）":"（経由）"}</li>)}</ol>
  {mapPickTarget!=="sequence"&&<button disabled={!enabled||saving} onClick={beginSequence}>駅の選択を再開</button>}
  {stale&&<button disabled={!enabled||loading||saving} onClick={()=>void search([...orderedRef.current])}>経路を再確認</button>}
  {loading&&<p role="status">経路を確認中…</p>}
  {error&&<p role="alert">{error}</p>}
  {result?.status==="no_path"&&<p role="status">接続する経路がありません。</p>}
  {(result?.truncated||result?.status==="incomplete")&&<p role="status">探索上限に達しました。経由駅を追加してください。</p>}
  {membership&&(entityLabel==="系統"?<ServiceStopDiagram label="停車駅" controlLabel={i=>`地図選択の停車駅 ${i+1}`} disabled onChange={()=>{}} rows={membership.member_stations.map((id,i)=>({key:String(i),name:stationName(id),stopping:i===0||i===membership.member_stations.length-1||(isStopping?.(id)??true)}))}/>:<p>所属駅: {membership.member_stations.map(stationName).join(" → ")}</p>)}
  {result&&!result.truncated&&result.status!=="incomplete"&&result.candidates.length>1&&<button disabled={!enabled||saving} onClick={()=>setShowChoices(true)}>経路を選択</button>}
  <div><button disabled={!enabled||!canApply||!membership||loading||saving||stale||!name.trim()||!onSave} onClick={()=>void finish()}>編集終了</button><button className="secondary" disabled={saving} onClick={()=>{invalidate();onMapPick?.(null);onDirty?.(false);onCancelEdit?.();}}>キャンセル</button></div>
  {showChoices&&result&&<div className="company-setup-backdrop"><section className="company-setup" role="dialog" aria-modal="true" aria-label={`${entityLabel}の経路を選択`}><h3>経路を選択</h3>
   {result.candidates.map((candidate,i)=><label key={i}><input type="radio" name="route-candidate" checked={chosen===i} disabled={!enabled||saving} onChange={()=>void selectCandidate(i)}/>{candidate.stations.map(stationName).join(" → ")}{i===result.shortest_candidate_index?"（最短）":""}</label>)}
   <button disabled={loading||saving||!membership} onClick={()=>setShowChoices(false)}>決定</button>
   <button className="secondary" disabled={saving} onClick={()=>setShowChoices(false)}>地図で選び直す</button>
   {error&&<p role="alert">{error}</p>}
  </section></div>}
 </section>;
}
