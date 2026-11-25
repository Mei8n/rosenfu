import {useCallback,useEffect,useRef} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {suggestRefugeBypass} from './refugeRouteSuggestion';
import type {RefugePlanningWorld} from './refugeRouteSuggestion';
import {RouteCandidatePreview} from './RouteCandidatePreview';
import type {Candidate,Candidates,Membership,StationSearchContext} from './RouteCandidatePreview';
import {resolveServicePath,sameOccurrence,sameSegment} from './servicePaths';
import type {PathWorld,PathPreview} from './servicePaths';
import type {CreateServicePath} from './ServicePathCreator';
/** Map-selected services own a path, not a newly-created route. Existing services are untouched. */
export function ServiceMapCreator({world,stations,enabled,defaultName,onDirty,onSave,onCancel,onMapPick,mapPickTarget,startSequenceKey,onSelectionChange,onCandidateChange,stationSearchDescription,onLocateStation}:StationSearchContext&{world:RefugePlanningWorld;stations:{id:string;name:string}[];enabled:boolean;defaultName:string;onDirty:(dirty:boolean)=>void;onSave:(draft:CreateServicePath)=>Promise<void>;onCancel:()=>void;onMapPick:(target:'start'|'end'|'via'|'sequence'|null,select?:(id:string)=>void)=>void;mapPickTarget:'start'|'end'|'via'|'sequence'|null;startSequenceKey:number;onSelectionChange?:(ids:string[])=>void;onCandidateChange?:(candidate:Candidate|null)=>void}){
 const current=useRef<PathWorld|null>(world);current.current=enabled?world:null;useEffect(()=>()=>{current.current=null;},[]);
 async function create(name:string,membership:Membership){
  const before=world,start=membership.path.stations[0],segments=membership.path.segments;
  const preview=await invoke<PathPreview>('preview_service_path',{start,segments});
  if(current.current!==before)throw Error('ワールドが変更されました。経路を選び直してください。');
  if(!Array.isArray(preview.legs)||preview.legs.length!==segments.length||preview.legs.some((leg,i)=>leg.key!==String(i+1)||!sameSegment(leg.segment,segments[i]))||!Array.isArray(preview.occurrences))throw Error('停車駅を確認できませんでした。');
  const expected=resolveServicePath(start,preview.legs,world);
  if(expected.length!==preview.occurrences.length||expected.some((entry,i)=>!sameOccurrence(entry,preview.occurrences[i])))throw Error('停車駅を確認できませんでした。');
  await onSave({name,start,segments,stops:preview.occurrences.map(o=>o.reference)});
 }
 const recommendCandidate=useCallback((result:Candidates)=>suggestRefugeBypass(result,world),[world]);
 return <RouteCandidatePreview stationSearchDescription={stationSearchDescription} onLocateStation={onLocateStation} recommendCandidate={recommendCandidate} onCandidateChange={onCandidateChange} onSelectionChange={onSelectionChange} entityLabel="系統" defaultName={defaultName} worldVersion={world} stations={stations} enabled={enabled} canApply={enabled} onSave={create} onDirty={onDirty} onCancelEdit={onCancel} onMapPick={onMapPick} mapPickTarget={mapPickTarget} startSequenceKey={startSequenceKey}/>;
}
