import {useEffect,useRef,useState} from 'react';
import type {GeoPoint} from './alignment';
import type {Camera} from './map';
import type {RoadSnapshot} from './automaticRoadData';
import {prepareOrdinaryBusQuery,snapOrdinaryBusVertex,verifyOrdinaryBusPreview,type BusAnchor,type BusEvidenceReader,type OrdinaryBusPreview,type OrdinaryBusQuery,type OrdinaryBusPattern,type RouteTileObserver} from './ordinaryBusDraft';

type Route={id:string;name:string;pattern?:OrdinaryBusPattern};
type Draft={id:string;pattern?:OrdinaryBusPattern;start?:BusAnchor;end?:BusAnchor;preview?:OrdinaryBusPreview;query?:OrdinaryBusQuery};
type Input={
 on:boolean;allow:boolean;scope:unknown;snapshot?:RoadSnapshot;loading:boolean;
 route?:Route;multiple:boolean;roundTrip:boolean;conflictingGraph:boolean;descriptors:readonly unknown[];
 observe?:RouteTileObserver;read:BusEvidenceReader;token:()=>Promise<string>;
 preview:(query:OrdinaryBusQuery)=>Promise<OrdinaryBusPreview>;
 commit:(query:OrdinaryBusQuery)=>Promise<unknown>;
 remove:(routeId:string)=>Promise<unknown>;
};
const displayError=(e:unknown)=>e instanceof Error?e.message:typeof e==='string'?e:'バス系統を変更できませんでした。';
export function useOrdinaryBusDraft(input:Input){
 const [draft,setDraft]=useState<Draft>();
 const [name,setName]=useState('バス系統');
 const [error,setError]=useState('');
 const [waiting,setWaiting]=useState(false);
 const [deleting,setDeleting]=useState(false);
 const job=useRef<AbortController|undefined>(undefined);
 const mutation=useRef(false);
 const current=useRef({on:input.on,scope:input.scope,allow:input.allow});
 current.current={on:input.on,scope:input.scope,allow:input.allow};
 function cancel(){
  job.current?.abort();job.current=undefined;setWaiting(false);setDraft(undefined);setError('');setDeleting(false);
 }
 useEffect(()=>{
  cancel();
  return ()=>{job.current?.abort();job.current=undefined;};
 },[input.on,input.scope]);
 function start(kind:'new'|'edit'){
  if(!input.on||!input.allow||input.conflictingGraph||mutation.current)return;
  const editing=kind==='edit'?input.route:undefined;
  if((kind==='edit'&&!editing)||(kind==='new'&&input.route&&!input.multiple))return;
  const pattern=editing?editing.pattern:input.roundTrip?'openRoundTrip':undefined;
  cancel();setDraft({id:editing?.id??`bus:${crypto.randomUUID()}`,pattern});
  setName(editing?.name??'バス系統');
 }
 async function pick(at:GeoPoint,camera:Camera,scale:number){
  if(!input.on||!input.allow||!draft||draft.preview||job.current||mutation.current)return;
  setError('');
  if(!input.snapshot){
   setError(input.loading?'道路を読み込み中です。':'道路を確認できませんでした。地図を拡大して選び直してください。');return;
  }
  let end:BusAnchor;
  try{end=snapOrdinaryBusVertex(input.snapshot,at,camera,scale,draft.start,!!input.observe);}
  catch(e){setError(displayError(e));return;}
  if(!draft.start){setDraft({...draft,start:end});return;}
  const start=draft.start;
  const title=name.trim();
  if(!title){setError('バス系統名を入力してください。');return;}
  const controller=new AbortController();
  const alive=()=>!controller.signal.aborted&&job.current===controller&&
   current.current.on&&current.current.scope===input.scope;
  job.current=controller;setWaiting(true);
  try{
   const token=await input.token();
   if(!alive())return;
   const query=await prepareOrdinaryBusQuery(start,end,title,draft.id,token,input.read,input.descriptors,controller.signal,input.observe,draft.pattern);
   if(!alive())return;
   const preview=verifyOrdinaryBusPreview(await input.preview(query),start,end,draft.pattern);
   if(alive())setDraft({...draft,end,preview,query});
  }catch(e){
   if(alive())setError(displayError(e));
  }finally{
   if(job.current===controller){job.current=undefined;setWaiting(false);}
  }
 }
 async function commit(){
  if(!input.on||!input.allow||!draft?.query||draft.pattern==="openRoundTrip"&&!draft.preview?.returnPath||waiting||mutation.current||!name.trim())return;
  mutation.current=true;setWaiting(true);setError('');
  try{await input.commit({...draft.query,name:name.trim()});cancel();}
  catch(e){if(current.current.on&&current.current.scope===input.scope)setError(displayError(e));}
  finally{mutation.current=false;setWaiting(false);}
 }
 async function remove(){
  if(!input.on||!input.allow||!input.route||!deleting||waiting||mutation.current)return;
  mutation.current=true;setWaiting(true);setError('');
  try{await input.remove(input.route.id);cancel();}
  catch(e){if(current.current.on&&current.current.scope===input.scope)setError(displayError(e));}
  finally{mutation.current=false;setWaiting(false);}
 }
 function selectDeleting(value:boolean){setError('');setDeleting(value);}
 return {draft,name,setName,error,waiting,deleting,selectDeleting,start,cancel,pick,commit,remove,mutating:()=>mutation.current,rectangle:!!input.observe};
}

export function OrdinaryBusPanel({state,route,routes,onSelect,multiple,blocked,enabled}:{
 state:ReturnType<typeof useOrdinaryBusDraft>;route?:Route;routes:readonly Route[];
 onSelect:(id:string)=>void;multiple:boolean;blocked:boolean;enabled:boolean;
}){
 const {draft,waiting,error}=state,disabled=!enabled||waiting;
 return <section aria-label="バス系統の建設">
  <h3>バス</h3>
  <p>地図の道路を使ってゲーム内の系統を作ります。実際の通行可否は示しません。</p>
  {blocked?<p>このワールドの別の道路交通データは上書きできません。</p>:<>
   {!draft&&!state.deleting&&(!route||multiple)&&<button type="button" disabled={disabled} onClick={()=>state.start('new')}>新しいバス系統</button>}
   {route&&(routes.length>1?<label>表示するバス系統<select value={route.id} disabled={disabled||!!draft||state.deleting}
    onChange={e=>onSelect(e.target.value)}>{routes.map(r=><option key={r.id} value={r.id}>{r.name}</option>)}</select></label>:<p>{route.name}</p>)}
   {route&&!draft&&!state.deleting&&<>
    <button type="button" disabled={disabled} onClick={()=>state.start('edit')}>バス系統を引き直す</button>
    <button type="button" disabled={disabled} onClick={()=>state.selectDeleting(true)}>系統を削除</button>
   </>}
   {draft&&<>
    <label>バス系統名<input maxLength={120} autoComplete="off" value={state.name} disabled={disabled} onChange={e=>state.setName(e.target.value)}/></label>
    <p role="status">{waiting?'道路経路を確認中':draft.preview?`道路経路 ${Math.round(draft.preview.lengthM)} m・停留所2か所`:draft.start?'道路上の終点を選択':'道路上の起点を選択'}</p>
    {draft.preview?.returnPath&&<p>復路 {Math.round(draft.preview.returnPath.lengthM)} m</p>}
    {draft.preview?.returnUnavailableReason&&<p role="alert">復路を作れません。{draft.preview.returnUnavailableReason}</p>}
    <p>{state.rectangle?"近い範囲の道路を経由します。遠い場合や範囲内に確認できない道路がある場合は選び直してください。":"この版では同じ取得範囲で起点と終点を選びます。"}停留所は両端に作られます。</p>
    {draft.preview&&<button type="button" onClick={()=>void state.commit()} disabled={disabled||!state.name.trim()||draft.pattern==="openRoundTrip"&&!draft.preview.returnPath}>バス系統を確定</button>}
    <button type="button" disabled={state.mutating()} onClick={state.cancel}>下書きを取り消す</button>
   </>}
   {state.deleting&&<div>
    <p>この系統と停留所を削除しますか？</p>
    <button type="button" disabled={disabled} onClick={()=>void state.remove()}>削除を確定</button>
    <button type="button" disabled={disabled} onClick={()=>state.selectDeleting(false)}>削除を取り消す</button>
   </div>}
  </>}
  {error&&<p role="alert">{error}</p>}
 </section>;
}
