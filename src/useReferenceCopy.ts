import {eligibleRoadGraph,validateRoadPicks} from './roadFollowing';
import {useEffect,useMemo,useRef,useState} from 'react';
import type {ReferenceGraphBundle} from './referenceGraphBundle';
import {createReferenceGraphRepository} from './referenceGraph';
import type {GraphPoint,GraphProjection,GraphMode} from './referenceGraph';
import type {ReferenceGraphPick} from './referenceGraphPath';
import {prepareReferenceCopy} from './referenceCopy';
import type {ReferenceBuild} from './referenceCopy';
export function useReferenceCopy({bundle,enabled,canStart,name,onBuild,roadFollow=false}:{roadFollow?:boolean;bundle:ReferenceGraphBundle|null;worldVersion:unknown;enabled:boolean;canStart:boolean;tool:string;name:string;onBuild:(build:ReferenceBuild)=>Promise<void>}){
 const [active,setActive]=useState(false),[mode,setMode]=useState<GraphMode>('rail'),[picks,setPicks]=useState<ReferenceGraphPick[]>([]),[redo,setRedo]=useState<ReferenceGraphPick[]>([]),[candidates,setCandidates]=useState<GraphProjection[]>([]),[message,setMessage]=useState(''),[confirmed,setConfirmed]=useState(false),[busy,setBusy]=useState(false);const generation=useRef(0),pending=useRef(false);
 const roadGraph=useMemo(()=>bundle&&roadFollow?eligibleRoadGraph(bundle):null,[bundle,roadFollow]);
 const repository=useMemo(()=>bundle?createReferenceGraphRepository(roadGraph??bundle.graph):null,[bundle,roadGraph]);
 const [hoverCandidates,setHoverCandidates]=useState<GraphProjection[]>([]);const hoverGeneration=useRef(0),lastHover=useRef(-Infinity);
 const hoverTimer=useRef<ReturnType<typeof setTimeout>|null>(null);
 function cancelHover(){hoverGeneration.current++;if(hoverTimer.current!==null){clearTimeout(hoverTimer.current);hoverTimer.current=null;}}
 function clearHover(){cancelHover();setHoverCandidates(old=>old.length?[]:old);}
 // Leading + trailing throttle: always query the last pointer position, even
 // when the pointer stops inside the 80ms window. New input invalidates older
 // async results immediately, not only when the next search begins.
 async function hover(point:GraphPoint){
  if(!enabled||!active||!roadFollow||!repository||pending.current)return;
  cancelHover();const token=hoverGeneration.current;
  const run=async()=>{hoverTimer.current=null;lastHover.current=Date.now();try{const r=await repository.nearest({point,mode:'road',radiusM:20,limit:20});if(token===hoverGeneration.current)setHoverCandidates(r.candidates);}catch{if(token===hoverGeneration.current)setHoverCandidates(old=>old.length?[]:old);}};
  const delay=Math.max(0,80-(Date.now()-lastHover.current));
  if(delay===0)await run();else hoverTimer.current=setTimeout(()=>{void run();},delay);
 }

 function clear(){clearHover();generation.current++;pending.current=false;setActive(false);setPicks([]);setRedo([]);setCandidates([]);setConfirmed(false);setBusy(false);}
 // Tool tabs and ordinary world snapshots do not own this session-local plan.
 // App confirms bundle replacement/removal, and explicitly clears on successful world load.
 useEffect(()=>{clear();setMessage('');return()=>{generation.current++;cancelHover();};},[bundle,roadFollow]);
 useEffect(()=>{if(!enabled)clearHover();},[enabled]);
 const plan=useMemo(()=>{if(!bundle||picks.length!==2)return {build:null,error:''};try{if(roadFollow)validateRoadPicks(bundle,picks);return {build:prepareReferenceCopy(bundle,picks[0],picks[1],name),error:''};}catch(e){return {build:null,error:String(e)};}},[bundle,picks,name,roadFollow]);
 function begin(){if(!enabled||!canStart||!repository)return;if(roadFollow&&!roadGraph?.edges.length){setMessage("利用できる道路がありません。検証済みローカル道路バンドルが必要です。");return;}clear();lastHover.current=-Infinity;setActive(true);if(roadFollow)setMode('road');setMessage('地図をクリックして起点候補を表示してください。');}
 async function click(point:GraphPoint){if(!enabled||!active||!repository||pending.current)return;clearHover();const token=++generation.current;pending.current=true;setBusy(true);setCandidates([]);setConfirmed(false);
 try{const result=await repository.nearest({point,mode:roadFollow?'road':mode,radiusM:20,limit:20});if(token!==generation.current)return;setCandidates(result.candidates);setMessage(result.status==='outside'?'バンドル範囲外です。':!result.candidates.length?'20m以内に候補がありません。実在しないことは意味しません。':`${result.total}候補。自動確定しません。${result.truncated?'表示は20候補まで。拡大して再選択してください。':''}${result.coverage==='boundary'?'範囲境界付近で網羅性はありません。':''}`);
 }catch(e){if(token===generation.current)setMessage(String(e));}finally{if(token===generation.current){pending.current=false;setBusy(false);}}}
 function choose(index:number){if(!enabled||pending.current||!active||!bundle)return;const candidate=candidates[index];if(!candidate)return;const first=picks[0]&&bundle.graph.edges.find(e=>e.id===picks[0].edgeId);if(first&&(first.osmWayId!==candidate.edge.osmWayId||first.mode!==candidate.edge.mode)){setMessage('終点は起点と同じway・種類から選んでください。');return;}
 const pick={datasetId:bundle.graph.dataset.id,revision:bundle.graph.dataset.revision,edgeId:candidate.edge.id,fraction:candidate.fraction};setPicks(old=>old.length?[old[0],pick]:[pick]);setRedo([]);setCandidates([]);setConfirmed(false);setMessage(first?'区間を確認して選択中の計画に追加してください。':'起点を選択しました。次に終点候補を表示してください。');}
 function undo(){if(!enabled||pending.current||!active)return;generation.current++;setCandidates([]);setConfirmed(false);if(picks.length){setRedo(old=>[...old,picks[picks.length-1]]);setPicks(old=>old.slice(0,-1));}}
 function redoPick(){if(!enabled||pending.current||!active||!redo.length)return;setPicks(old=>[...old,redo[redo.length-1]]);setRedo(old=>old.slice(0,-1));setCandidates([]);setConfirmed(false);}
 async function build(){if(!enabled||pending.current||!active||!plan.build||!confirmed||!name.trim())return;const token=generation.current;pending.current=true;setBusy(true);try{await onBuild(plan.build);if(token===generation.current){clear();setMessage('実在形状に沿う経路を計画に追加しました。');}}catch(e){if(token===generation.current)setMessage(String(e));}finally{if(token===generation.current){pending.current=false;setBusy(false);}}}
 return {active,mode,picks,redo,candidates,roadFollow,roadAvailable:!!roadGraph?.edges.length,hoverCandidates,hover,clearHover,message,confirmed,setConfirmed,busy,plan,begin,clear,click,choose,undo,redoPick,build,setMode:(value:GraphMode)=>{if(!enabled||pending.current||roadFollow||picks.length||redo.length)return;generation.current++;setMode(value);setPicks([]);setRedo([]);setCandidates([]);setConfirmed(false);}};
}
export type ReferenceCopyState=ReturnType<typeof useReferenceCopy>;
