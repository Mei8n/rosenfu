import {editStationOrder,replaceStationEndpoint} from './mapStationEdit';
import {ServiceStopDiagram} from './ServiceStopDiagram';
import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {EndpointConnection} from "./destinationJoin";
export type Segment = { kind: "alignment"; alignment_id: string; from: string; to: string } | {kind:"connected_alignments";from:string;to:string;connections:EndpointConnection[]};
export type Candidate = { stations: string[]; segments: Segment[] };
export type Candidates = { shortest_candidate_index?: number | null; status: "no_path" | "unique" | "ambiguous" | "incomplete"; candidates: Candidate[]; truncated: boolean };
export type Membership = { path: Candidate; member_stations: string[]; excluded_stations: string[] };
export type StationSearchContext={
 stationSearchDescription?: ReadonlyMap<string,string>;
 onLocateStation?: (id:string)=>void;
};
const normalizeStationName=(text:string)=>text.normalize('NFKC').trim().toLowerCase();
type EndpointPick = {side:"start"|"end"; state:"idle"|"queued"|"running"};
type Props = StationSearchContext & {
  entityLabel?: string;
  onStationClick?: (id:string)=>boolean;
  isStopping?: (id:string)=>boolean;
  recommendCandidate?: (result:Candidates)=>{index:number;stopId:string}|null;
  candidateLength?: (candidate:Candidate)=>number;
  onCandidateChange?: (candidate:Candidate|null) => void;
  onSelectionChange?: (stationIds: string[]) => void;
  editRoute?: {id:string;name:string;path?:Candidate;excludedStations?:string[];memberStations?:string[]}; defaultName?:string;
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
export function RouteCandidatePreview({onStationClick,isStopping,candidateLength,recommendCandidate,onCandidateChange,onSelectionChange,entityLabel="路線",editRoute,defaultName="路線1",onCancelEdit,startSequenceKey,mapPickTarget,onMapPick,stations,worldVersion,resetKey,onDirty,enabled,canApply,onSave,stationSearchDescription,onLocateStation}:Props){
 const initialName=editRoute?.name??defaultName;
 const [name,setName]=useState(initialName);
 const [stationQuery,setStationQuery]=useState("");
 const [ordered,setOrdered]=useState<string[]>([]);
 const editingMap=!!editRoute?.path;
 const stationClick=useRef(onStationClick);stationClick.current=onStationClick;
 const editQueue=useRef(Promise.resolve());
 const endpointPick=useRef<EndpointPick|null>(null);
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
 const [choiceRequired,setChoiceRequired]=useState(false);
 const recommendation=useMemo(()=>result?recommendCandidate?.(result):null,[result,recommendCandidate]);
 const generation=useRef(0), pendingSave=useRef(false);
 const blocked=useRef(false);blocked.current=!enabled||stale;
 const names=useMemo(()=>new Map(stations.map(s=>[s.id,s.name])),[stations]);
 const searchableStations=useMemo(()=>{
  const counts=new Map<string,number>(),descriptions=new Map<string,number>();
  const rows=stations.map(station=>{
   const name=normalizeStationName(station.name),description=stationSearchDescription?.get(station.id)??'';
   counts.set(name,(counts.get(name)??0)+1);
   const key=JSON.stringify([name,description]);
   descriptions.set(key,(descriptions.get(key)??0)+1);
   return {station,name,description,key};
  });
  const ordinals=new Map<string,number>();
  return rows.map(({station,name,description,key})=>{
   const duplicate=(counts.get(name)??0)>1;
   const ordinal=(ordinals.get(key)??0)+1;ordinals.set(key,ordinal);
   const context=duplicate?[description,(descriptions.get(key)??0)>1?`候補 ${ordinal}`:''].filter(Boolean).join(' / '):'';
   return {station,name,context,duplicate};
  });
 },[stations,stationSearchDescription]);
 const stationSearch=useMemo(()=>{
  const query=normalizeStationName(stationQuery);
  if(!query)return {rows:[],count:0};
  const exact:typeof searchableStations=[],partial:typeof searchableStations=[];
  for(const row of searchableStations){
   if(row.name===query)exact.push(row);
   else if(row.name.includes(query))partial.push(row);
  }
  return {rows:exact.concat(partial).slice(0,24),count:exact.length+partial.length};
 },[stationQuery,searchableStations]);
 const editingInputs=useRef({names,candidateLength});editingInputs.current={names,candidateLength};
 const stationName=(id:string)=>names.get(id)??`駅 ${id}`;
 function clearEndpoint(){if(endpointPick.current?.state==="running")setLoading(false);endpointPick.current=null;}
 useEffect(()=>{if(mapPickTarget===null)clearEndpoint();},[mapPickTarget]);
 function invalidate(){generation.current++;if(endpointPick.current)endpointPick.current.state="idle";setResult(null);setMembership(null);setChosen(null);setLoading(false);setStale(false);setShowChoices(false);setChoiceRequired(false);setError("");}
 useEffect(()=>{invalidate();orderedRef.current=[];setOrdered([]);setName(initialName);setStationQuery("");},[resetKey]);
 useEffect(()=>{generation.current++;if(endpointPick.current)endpointPick.current.state="idle";setLoading(false);setMembership(null);setShowChoices(false);setStale(orderedRef.current.length>1);return()=>{generation.current++;};},[worldVersion]);
 useEffect(()=>{onDirty?.(ordered.length>0||name!==initialName);},[ordered,name,initialName,onDirty]);
 async function membershipFor(candidate:Candidate){
  const args={start:candidate.stations[0],segments:candidate.segments};
  const all=await invoke<Membership>("preview_route_membership",{...args,excludedStations:[]});
  const endpoints=new Set([all.path.stations[0],all.path.stations.at(-1)]);
  const excluded=(editRoute?.excludedStations??[]).filter(id=>all.member_stations.includes(id)&&!endpoints.has(id));
  return excluded.length?invoke<Membership>("preview_route_membership",{...args,excludedStations:excluded}):all;
 }
 async function validate(candidate:Candidate,token:number){
  const next=await membershipFor(candidate);
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
   const suggested=recommendCandidate?.(response);
   setChoiceRequired(!!suggested);
   const shortest=response.shortest_candidate_index;
   const index=suggested?.index??(Number.isInteger(shortest)&&shortest!>=0&&shortest!<response.candidates.length?shortest!:response.status==="unique"&&response.candidates.length===1?0:null);
   if(index!==null){setChosen(index);await validate(response.candidates[index],token);}
   if(token===generation.current&&response.candidates.length>1)setShowChoices(true);
  }catch(cause){if(token===generation.current)setError(String(cause));}
  finally{if(token===generation.current)setLoading(false);}
 }
 const lookup=(ids:string[])=>invoke<Candidates>('preview_route_candidates',{start:ids[0],end:ids.at(-1),...(ids.length>2?{viaStationIds:ids.slice(1,-1)}:{})});
 function orderedChoice(ids:string[],result:Candidates){
  if(result.truncated||result.status==='incomplete')throw Error('経路を確認しきれませんでした。元の編集内容を保持しています');
  if(result.status==='no_path'||!result.candidates.length)throw Error('この順序では接続できません。元の編集内容を保持しています');
  const shortest=result.shortest_candidate_index;
  const index=Number.isInteger(shortest)&&shortest!>=0&&shortest!<result.candidates.length?shortest!:result.candidates.length===1?0:null;
  return {ids,result,index};
 }
 function applyMapEdit(id:string,endpoint?:EndpointPick,reorder?:{direction:-1|1}){
  const session=generation.current;
  const current=()=>session===generation.current&&(!endpoint||endpointPick.current===endpoint);
  editQueue.current=editQueue.current.then(async()=>{
   if(!current()||pendingSave.current||blocked.current){if(endpoint)endpoint.state="idle";return;}
   let accepted=false;
   if(endpoint)endpoint.state="running";
   setLoading(true);setError("");
   try{
    let choice:{ids:string[];result:Candidates;index:number|null};
    if(reorder){
     const ids=[...orderedRef.current],from=ids.indexOf(id),to=from+reorder.direction;
     if(from<1||from>=ids.length-1||to<1||to>=ids.length-1)return;
     [ids[from],ids[to]]=[ids[to],ids[from]];
     choice=orderedChoice(ids,await lookup(ids));
    }else if(endpoint){
     const ids=replaceStationEndpoint(orderedRef.current,endpoint.side,id);
     if(ids.every((station,i)=>station===orderedRef.current[i])&&ids.length===orderedRef.current.length)return;
     const result=await lookup(ids);
     if(!result.truncated&&result.status!=='incomplete'&&(result.status==='no_path'||!result.candidates.length))throw Error('この駅には接続できません。元の編集内容を保持しています');
     choice=orderedChoice(ids,result);
    }else{
     const length=editingInputs.current.candidateLength;
     if(!length)throw Error('経路の比較に必要な地図情報がありません');
     choice=await editStationOrder(orderedRef.current,id,lookup,length);
    }
    if(!current())return;
    const next=choice.index===null?null:await membershipFor(choice.result.candidates[choice.index]);
    if(!current())return;
    orderedRef.current=choice.ids;setOrdered(choice.ids);setResult(choice.result);setChosen(choice.index);setMembership(next);setStale(false);setShowChoices(choice.result.candidates.length>1);accepted=true;
   }catch(cause){if(current())setError(String(cause));}
   finally{if(current()){if(endpoint)endpoint.state="idle";setLoading(false);}}
   if(accepted&&endpoint&&current())beginSequence();
  });
 }
 function beginEndpoint(side:'start'|'end'){
  clearEndpoint();
  const pick:EndpointPick={side,state:"idle"};endpointPick.current=pick;
  onMapPick?.(side,id=>selectEndpoint(id,pick));
 }
 function selectEndpoint(id:string,pick=endpointPick.current){
   if(!pick)return;
   if(endpointPick.current!==pick||pick.state!=="idle"||pendingSave.current||blocked.current||!editingInputs.current.names.has(id))return;
   pick.state="queued";applyMapEdit(id,pick);
 }
 function beginSequence(){
  clearEndpoint();
  onMapPick?.("sequence",selectSequence);
 }
 function selectSequence(id:string){
   if(pendingSave.current||blocked.current||!editingInputs.current.names.has(id))return;
   if(editingMap){
    if(id===orderedRef.current[0]||id===orderedRef.current.at(-1)){beginEndpoint(id===orderedRef.current[0]?'start':'end');return;}
    if(!stationClick.current?.(id))applyMapEdit(id);
    return;
   }
   if(orderedRef.current.includes(id)){setError("同じ駅は重複して指定できません。");return;}
   const next=[...orderedRef.current,id];orderedRef.current=next;setOrdered(next);void search(next);
 }
 function beginEditing(){
  const path=editRoute?.path;if(!path)return;
  if(new Set(path.stations).size!==path.stations.length){setError('同じ駅を複数回通る経路の地図編集には未対応です。区間編集を利用してください');return;}
  invalidate();orderedRef.current=[...path.stations];setOrdered([...path.stations]);setMembership({path,member_stations:editRoute?.memberStations??path.stations,excluded_stations:editRoute?.excludedStations??[]});beginSequence();
 }
 useEffect(()=>{if(startSequenceKey){if(editingMap)beginEditing();else beginSequence();}},[startSequenceKey]);
 async function selectCandidate(index:number){
  if(!enabled||saving||stale||!result||result.truncated||result.status==="incomplete"||!result.candidates[index])return;
  const token=++generation.current;setChosen(index);setMembership(null);setLoading(true);setError("");
  try{await validate(result.candidates[index],token);}catch(cause){if(token===generation.current)setError(String(cause));}
  finally{if(token===generation.current)setLoading(false);}
 }
 async function finish(){
  if(!enabled||!canApply||!onSave||!membership||choiceRequired||loading||stale||pendingSave.current||!name.trim())return;
  pendingSave.current=true;setSaving(true);setError("");
  try{await onSave(name.trim(),membership);onMapPick?.(null);onDirty?.(false);}
  catch(cause){setError(String(cause));}
  finally{pendingSave.current=false;setSaving(false);}
 }
 return <section className="route-candidate-preview" aria-label={editRoute?`${entityLabel}を編集`:`${entityLabel}を作成`}>
  <h4>{name||initialName}</h4>
  <p className="muted">編集終了でワールドに適用します。ファイルに残すには「保存」が必要です。</p>
  <label>{entityLabel}名<input autoComplete="off" value={name} maxLength={80} disabled={!enabled||saving} onChange={e=>setName(e.target.value)}/></label>
  {editingMap&&<p>{entityLabel==='系統'?'中間駅をクリックすると停車・通過を切り替えます。':'中間駅をクリックすると経由指定を追加・解除できます。'}始点・終点を変更するには、地図の端の駅をクリックしてから新しい駅を選んでください。</p>}
  {editingMap&&(mapPickTarget==="start"||mapPickTarget==="end")&&<p role="status">地図か駅名検索で新しい{mapPickTarget==="start"?"始点":"終点"}を選択してください。</p>}
  {(mapPickTarget==="sequence"||mapPickTarget==="start"||mapPickTarget==="end")&&<div className="route-station-search">
   <label>駅名で検索<input type="search" value={stationQuery} autoComplete="off" maxLength={80} disabled={!enabled||saving||stale}
    onChange={e=>setStationQuery(e.target.value)}/></label>
   {stationQuery.trim()!==""&&<p className="muted" aria-live="polite">{stationSearch.count===0?'一致する駅がありません':`${stationSearch.count}件${stationSearch.count>24?'（先頭24件を表示）':''}`}</p>}
   {stationSearch.rows.length>0&&<ul aria-label="駅名検索結果">{stationSearch.rows.map(({station,context,duplicate})=><li key={station.id}>
    <button type="button" className="secondary" disabled={!enabled||saving||stale||(mapPickTarget!=="sequence"&&loading)}
     onClick={()=>{if(mapPickTarget==="sequence")selectSequence(station.id);else selectEndpoint(station.id);setStationQuery("");}}>{station.name}{context&&<> <small>{context}</small></>}</button>
    {duplicate&&onLocateStation&&<button type="button" className="secondary route-station-locate" aria-label={`${station.name} ${context}を地図で確認`}
     onClick={()=>onLocateStation(station.id)}>地図で確認</button>}
   </li>)}</ul>}
  </div>}
  <ol aria-label="選択した駅">{ordered.map((id,i)=>{
   const side=i===0?'start':i===ordered.length-1?'end':null;
   const role=side==='start'?'始点':side==='end'?'終点':'経由';
   return <li key={id}>{editingMap&&side?<button type="button" className="secondary" disabled={!enabled||saving||loading||stale} aria-pressed={mapPickTarget===side} onClick={()=>mapPickTarget===side?beginSequence():beginEndpoint(side)}>{role}を変更: {stationName(id)}</button>:<>{stationName(id)}（{role}）</>}
    {!side&&ordered.length>=4&&<span className="route-via-actions">
     <button type="button" className="secondary" disabled={!enabled||saving||loading||stale||i===1} aria-label={stationName(id)+'を一つ前へ'} onClick={()=>applyMapEdit(id,undefined,{direction:-1})}>↑</button>
     <button type="button" className="secondary" disabled={!enabled||saving||loading||stale||i===ordered.length-2} aria-label={stationName(id)+'を一つ後ろへ'} onClick={()=>applyMapEdit(id,undefined,{direction:1})}>↓</button>
    </span>}</li>;
  })}</ol>
  {mapPickTarget!=="sequence"&&<button disabled={!enabled||saving} onClick={beginSequence}>{mapPickTarget?"端点の選択をやめる":"駅の選択を再開"}</button>}
  {stale&&<button disabled={!enabled||loading||saving} onClick={()=>void search([...orderedRef.current])}>経路を再確認</button>}
  {loading&&<p role="status">経路を確認中…</p>}
  {error&&<p role="alert">{error}</p>}
  {result?.status==="no_path"&&<p role="status">接続する経路がありません。</p>}
  {(result?.truncated||result?.status==="incomplete")&&<p role="status">探索上限に達しました。経由駅を追加してください。</p>}
  {membership&&(entityLabel==="系統"?<ServiceStopDiagram label="停車駅" controlLabel={i=>`地図選択の停車駅 ${i+1}`} disabled onChange={()=>{}} rows={membership.member_stations.map((id,i)=>({key:String(i),name:stationName(id),stopping:i===0||i===membership.member_stations.length-1||(isStopping?.(id)??true)}))}/>:<p>所属駅: {membership.member_stations.map(stationName).join(" → ")}</p>)}
  {result&&!result.truncated&&result.status!=="incomplete"&&result.candidates.length>1&&<button disabled={!enabled||saving} onClick={()=>setShowChoices(true)}>経路を選択</button>}
  <div><button disabled={!enabled||!canApply||!membership||choiceRequired||loading||saving||stale||!name.trim()||!onSave} onClick={()=>void finish()}>編集終了</button><button className="secondary" disabled={saving} onClick={()=>{invalidate();onMapPick?.(null);onDirty?.(false);onCancelEdit?.();}}>キャンセル</button></div>
  {showChoices&&result&&<div className="company-setup-backdrop"><section className="company-setup" role="dialog" aria-modal="true" aria-label={`${entityLabel}の経路を選択`}><h3>経路を選択</h3>
   {recommendation&&<p>既存の停車系統が使う{stationName(recommendation.stopId)}の枝を避ける経路を候補にしました。運行時に待避できるとは限りません。</p>}
   {result.candidates.map((candidate,i)=><label key={i}><input type="radio" name="route-candidate" checked={chosen===i} disabled={!enabled||saving} onChange={()=>void selectCandidate(i)}/>{candidate.stations.map(stationName).join(" → ")}{i===result.shortest_candidate_index?"（最短）":""}{i===recommendation?.index?"（停車駅の枝を避ける）":""}</label>)}
   <button disabled={!enabled||stale||loading||saving||!membership} onClick={()=>{setChoiceRequired(false);setShowChoices(false);}}>決定</button>
   <button className="secondary" disabled={saving} onClick={()=>setShowChoices(false)}>地図で選び直す</button>
   {error&&<p role="alert">{error}</p>}
  </section></div>}
 </section>;
}
