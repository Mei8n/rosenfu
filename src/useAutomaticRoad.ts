import type {OpenFreeMapEvidenceRequest,OpenFreeMapEvidenceBatch} from './openFreeMapRoadData';
import {useCallback,useEffect,useRef,useState} from 'react';
import {createRoadProvider,type RoadProvider} from './roadProvider';
import {roadBoundsKey,ROAD_LIMITS,type RoadDataState,type RoadSnapshot} from './automaticRoadData';
import type {GraphBounds} from './referenceGraph';
let provider:RoadProvider|undefined;
type SharedLoad={controller:AbortController;promise:Promise<RoadSnapshot>;users:number};
const pending=new Map<string,SharedLoad>();
const cancelled=()=>new DOMException('道路取得を取り消しました。','AbortError');
async function load(area:GraphBounds,signal:AbortSignal){
 if(signal.aborted)throw signal.reason??cancelled();
 const key=roadBoundsKey(area);
 let task=pending.get(key);
 if(!task){
  provider??=createRoadProvider();
  const controller=new AbortController();
  const created:SharedLoad={controller,users:0,promise:provider.load(area,controller.signal).finally(()=>{
   if(pending.get(key)===created)pending.delete(key);
  })};
  task=created;pending.set(key,task);
 }
 task.users++;
 let listening=true;
 const release=()=>{
  if(!listening)return;
  listening=false;
  if(--task.users===0&&pending.get(key)===task){
   // Unlink before aborting: a newer request may reuse this viewport,
   // and a delayed stale completion must not delete that replacement.
   pending.delete(key);
   task.controller.abort();
  }
 };
 let stop:()=>void=()=>{};
 try{
  const stopped=new Promise<never>((_,reject)=>{
   stop=()=>{release();reject(signal.reason??cancelled());};
   signal.addEventListener('abort',stop,{once:true});
   if(signal.aborted)stop();
  });
  return await Promise.race([task.promise,stopped]);
 }finally{
  signal.removeEventListener('abort',stop);
  release();
 }
}
export type CurrentRoadEvidenceResult={status:'ready';evidence:OpenFreeMapEvidenceBatch}|{status:'unavailable';reason:string}|{status:'stale'};
export type CurrentRoadResult={status:'ready';snapshot:RoadSnapshot}|{status:'unavailable';reason:string}|{status:'stale'};
/** Road display/following is independent of water availability. No construction
 * approval or crossing assessment is implied by a successfully loaded reference. */
export function useAutomaticRoad(bounds:GraphBounds,enabled:boolean,world:unknown,frozen=false){
 const view={west:Math.floor(bounds.west*200)/200,south:Math.floor(bounds.south*200)/200,east:Math.ceil(bounds.east*200)/200,north:Math.ceil(bounds.north*200)/200};
 const key=JSON.stringify(view);
 const [result,setResult]=useState<{key:string;world:unknown;state:RoadDataState}>();
 const [attempt,setAttempt]=useState(0);
 const current=useRef({key,world,enabled});current.current={key,world,enabled};
 const debounce=useRef<ReturnType<typeof setTimeout>|null>(null);
 useEffect(()=>()=>{current.current.enabled=false;},[]);
 useEffect(()=>{
  if(!enabled||frozen)return;let active=true;const controller=new AbortController();
  const publish=(state:RoadDataState)=>{if(active)setResult({key,world,state});};
  const area=JSON.parse(key) as GraphBounds;
  try{roadBoundsKey(area);}catch(error){publish({status:'unavailable',message:String(error)});return;}
  publish({status:'loading',message:'道路データ取得中'});
  const timer=setTimeout(()=>{debounce.current=null;void(async()=>{try{
   const snapshot=await load(area,controller.signal);
   publish({status:'ready',snapshot,message:`道路参考データ取得済み（${provider!.name}）。現実の全道路の網羅性は保証しません。`});
  }catch(error){publish({status:'error',message:`道路参考データ未取得：${String(error)}`});}})();},300);
  debounce.current=timer;
  return()=>{active=false;controller.abort();clearTimeout(timer);if(debounce.current===timer)debounce.current=null;};
 },[key,enabled,world,frozen,attempt]);
 const state=enabled&&result&&result.world===world&&(result.key===key||frozen)?result.state:{status:'unavailable' as const,message:'道路参考データ未取得'};
 async function ensureCurrent(signal?:AbortSignal):Promise<CurrentRoadResult>{
  const matches=()=>!signal?.aborted&&current.current.enabled&&current.current.key===key&&current.current.world===world;
  if(!matches())return {status:'stale'};
  if(state.status==='ready'&&state.snapshot)return {status:'ready',snapshot:state.snapshot};
  if(debounce.current!==null){clearTimeout(debounce.current);debounce.current=null;}
  const controller=new AbortController();
  const abort=()=>controller.abort(signal?.reason??cancelled());
  signal?.addEventListener('abort',abort,{once:true});
  if(signal?.aborted)abort();
  // Deadline includes provider queue time. Expiring this caller only
  // cancels the work if no current viewport or other build still needs it.
  const timer=setTimeout(()=>controller.abort(Error('道路確認が25秒で完了しませんでした')),ROAD_LIMITS.timeoutMs);
  try{
   const area=JSON.parse(key) as GraphBounds;
   const snapshot=await load(area,controller.signal);
   if(!matches())return {status:'stale'};
   setResult({key,world,state:{status:'ready',snapshot,message:`道路参考データ取得済み（${provider!.name}）。現実の全道路の網羅性は保証しません。`}});
   return {status:'ready',snapshot};
  }catch(error){
   if(!matches())return {status:'stale'};
   const reason=`道路未確認：${String(error)}`;setResult({key,world,state:{status:'error',message:reason}});return {status:'unavailable',reason};
  }finally{clearTimeout(timer);signal?.removeEventListener('abort',abort);}
 }
 const readTileEvidenceBatch=useCallback(async(refs:readonly OpenFreeMapEvidenceRequest[],signal?:AbortSignal):Promise<CurrentRoadEvidenceResult>=>{
  const matches=()=>!signal?.aborted&&current.current.enabled&&current.current.key===key&&current.current.world===world;
  if(!matches())return {status:'stale'};
  // Reuse the exact provider that observed these tiles. A new repository would
  // lose its pinned knownRaw map and must not silently establish new evidence.
  if(!provider)return {status:'unavailable',reason:'道路原データはまだ取得されていません。'};
  try{const evidence=await provider.readTileEvidenceBatch(refs,signal);return matches()?{status:'ready',evidence}:{status:'stale'};}
  catch(error){return matches()?{status:'unavailable',reason:`道路証拠未取得：${String(error)}`}:{status:'stale'};}
 },[key,world,enabled]);
 const readPinnedSeamTiles=useCallback(async(refs:readonly OpenFreeMapEvidenceRequest[],signal?:AbortSignal):Promise<CurrentRoadEvidenceResult>=>{
  const matches=()=>!signal?.aborted&&current.current.enabled&&current.current.key===key&&current.current.world===world;
  if(!matches())return {status:'stale'};
  if(!provider)return {status:'unavailable',reason:'道路データの提供元がありません。'};
  try{
   const evidence=await provider.readPinnedSeamTiles(refs,signal);
   return matches()?{status:'ready',evidence}:{status:'stale'};
  }catch(error){
   return matches()?{status:'unavailable',reason:`確認済みの版を取得できません：${String(error)}`}:{status:'stale'};
  }
 },[key,world,enabled]);
 const observeRouteTiles=useCallback(async(cells:readonly (readonly [number,number])[],revision:string,signal?:AbortSignal)=>{
  const matches=()=>!signal?.aborted&&current.current.enabled&&current.current.key===key&&current.current.world===world;
  if(!matches()||!provider)throw Error('道路取得が終了しました。位置を選び直してください。');
  const observed=await provider.observeTiles(cells,revision,signal);
  if(!matches())throw Error('道路取得範囲が変わりました。位置を選び直してください。');
  return observed;
 },[key,world,enabled]);
 return {...state,readTileEvidenceBatch,readPinnedSeamTiles,observeRouteTiles,retry:()=>setAttempt(n=>n+1),ensureCurrent};
}
