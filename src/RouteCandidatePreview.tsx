import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {EndpointConnection} from "./destinationJoin";
export type Segment = { kind: "alignment"; alignment_id: string; from: string; to: string } | {kind:"connected_alignments";from:string;to:string;connections:EndpointConnection[]};
export type Candidate = { stations: string[]; segments: Segment[] };
export type Candidates = { shortest_candidate_index?: number | null; status: "no_path" | "unique" | "ambiguous" | "incomplete"; candidates: Candidate[]; truncated: boolean };
export type Membership = { path: Candidate; member_stations: string[]; excluded_stations: string[] };
type Props = {
  entityLabel?: string;
  editRoute?: {id:string;name:string}; defaultName?:string;
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
export function RouteCandidatePreview({entityLabel="路線",editRoute,defaultName="路線1",onCancelEdit,startSequenceKey,mapPickTarget,onMapPick,stations,worldVersion,resetKey,onDirty,enabled,canApply,onSave}:Props){
 const initialName=editRoute?.name??defaultName;
 const [name,setName]=useState(initialName);
 const [ordered,setOrdered]=useState<string[]>([]);
 const orderedRef=useRef<string[]>([]);
 const [result,setResult]=useState<Candidates|null>(null);
 const [chosen,setChosen]=useState<number|null>(null);
 const [membership,setMembership]=useState<Membership|null>(null);
 const [error,setError]=useState("");
 const [loading,setLoading]=useState(false);
 const [saving,setSaving]=useState(false);
 const [stale,setStale]=useState(false);
 const [showChoices,setShowChoices]=useState(false);
 const generation=useRef(0), pendingSave=useRef(false);
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
   if(pendingSave.current||!names.has(id))return;
   if(orderedRef.current.includes(id)){setError("同じ駅は重複して指定できません。");return;}
   const next=[...orderedRef.current,id];orderedRef.current=next;setOrdered(next);void search(next);
  });
 }
 useEffect(()=>{if(startSequenceKey)beginSequence();},[startSequenceKey]);
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
  <ol aria-label="選択した駅">{ordered.map((id,i)=><li key={id}>{stationName(id)}{i===0?"（始点）":i===ordered.length-1?"（終点）":"（経由）"}</li>)}</ol>
  {mapPickTarget!=="sequence"&&<button disabled={!enabled||saving} onClick={beginSequence}>駅の選択を再開</button>}
  {stale&&<button disabled={!enabled||loading||saving} onClick={()=>void search([...orderedRef.current])}>経路を再確認</button>}
  {loading&&<p role="status">経路を確認中…</p>}
  {error&&<p role="alert">{error}</p>}
  {result?.status==="no_path"&&<p role="status">接続する経路がありません。</p>}
  {(result?.truncated||result?.status==="incomplete")&&<p role="status">探索上限に達しました。経由駅を追加してください。</p>}
  {membership&&<p>{entityLabel==="系統"?"停車駅":"所属駅"}: {membership.member_stations.map(stationName).join(" → ")}</p>}
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
