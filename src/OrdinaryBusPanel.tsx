import {useEffect,useRef,useState} from 'react';
import {useOrdinaryBusInteraction} from './useOrdinaryBusInteraction';
import type {GeoPoint} from './alignment';
import type {Camera} from './map';
import type {RoadSnapshot} from './automaticRoadData';
import {prepareOrdinaryBusQuery,snapOrdinaryBusVertex,verifyOrdinaryBusPreview,type BusAnchor,type BusEvidenceReader,type OrdinaryBusPreview,type OrdinaryBusPreviewQuery,type OrdinaryBusPreviewReply,type OrdinaryBusPath,type OrdinaryBusQuery,type OrdinaryBusPattern,type RouteTileObserver} from './ordinaryBusDraft';

type Route={id:string;name:string;pattern?:OrdinaryBusPattern;path?:OrdinaryBusPath};
type Draft={id:string;respectOneway:boolean;pattern?:OrdinaryBusPattern;start?:BusAnchor;end?:BusAnchor;preview?:OrdinaryBusPreview;query?:OrdinaryBusQuery};
type Input={
 on:boolean;allow:boolean;scope:unknown;snapshot?:RoadSnapshot;loading:boolean;
 route?:Route;multiple:boolean;roundTrip:boolean;waypoints?:boolean;autoStopsAvailable?:boolean;closedCapable?:boolean;observedGrade?:boolean;conflictingGraph:boolean;descriptors:readonly unknown[];
 observe?:RouteTileObserver;read:BusEvidenceReader;token:()=>Promise<string>;
 preview:(query:OrdinaryBusPreviewQuery)=>Promise<OrdinaryBusPreviewReply>;
 commit:(query:OrdinaryBusQuery)=>Promise<unknown>;
 remove:(routeId:string)=>Promise<unknown>;
};
const displayError=(e:unknown)=>e instanceof Error?e.message:typeof e==='string'?e:'バス系統を変更できませんでした。';
export function useOrdinaryBusDraft(input:Input){
 const [draft,setDraft]=useState<Draft>();
 const [autoStops,setAutoStops]=useState(false);
 const [newRoute,setNewRoute]=useState(false);
 const [respectOneway,setRespectOneway]=useState(false);
 const [name,setName]=useState('バス系統');
 const [error,setError]=useState('');
 const [waiting,setWaiting]=useState(false);
 const [deleting,setDeleting]=useState(false);
 const job=useRef<AbortController|undefined>(undefined);
 const mutation=useRef(false);
 const current=useRef({on:input.on,scope:input.scope,allow:input.allow});
 current.current={on:input.on,scope:input.scope,allow:input.allow};
 const waypoints=useOrdinaryBusInteraction({...input,active:input.on&&!!input.waypoints,autoRoute:!!input.autoStopsAvailable&&autoStops&&newRoute},name,()=>cancel());
 function cancel(){
  waypoints.reset();
  job.current?.abort();job.current=undefined;setWaiting(false);setDraft(undefined);setError('');setDeleting(false);
 }
 useEffect(()=>{
  cancel();setRespectOneway(false);
  return ()=>{job.current?.abort();job.current=undefined;};
 },[input.on,input.scope]);
 function start(kind:'new'|'edit'){
  if(!input.on||!input.allow||input.conflictingGraph||mutation.current)return;
  const editing=kind==='edit'?input.route:undefined;
  if((kind==='edit'&&!editing)||(kind==='new'&&input.route&&!input.multiple))return;
  const pattern=editing?editing.pattern:input.roundTrip?'openRoundTrip':undefined;
  cancel();const spec={id:editing?.id??`bus:${crypto.randomUUID()}`,pattern,
   respectOneway:editing?editing.path?.respectOneway??false:respectOneway};setDraft(spec);
  if(input.waypoints)waypoints.start({...spec,savedPath:editing?.path,
   gradePolicy:editing?editing.path?.gradePolicy:input.observedGrade?'observedConnectors':undefined});
  setNewRoute(!editing);setAutoStops(!editing&&!!input.autoStopsAvailable);
  setName(editing?.name??'バス系統');
 }
 async function pick(at:GeoPoint,camera:Camera,scale:number,detail=1,handleId?:string,moveId?:string,afterId?:string){
  if(input.waypoints){waypoints.point(at,camera,scale,detail,handleId,moveId,afterId);return;}
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
   const query=await prepareOrdinaryBusQuery(start,end,title,draft.id,token,input.read,input.descriptors,controller.signal,input.observe,draft.pattern,[],draft.respectOneway);
   if(!alive())return;
   const result=await input.preview(query);
   if('failedLegIndex' in result)throw Error('この区間を道路でつなげません。');
   const preview=verifyOrdinaryBusPreview(result,start,end,draft.pattern);
   if(alive())setDraft({...draft,end,preview,query});
  }catch(e){
   if(alive())setError(displayError(e));
  }finally{
   if(job.current===controller){job.current=undefined;setWaiting(false);}
  }
 }
 async function commit(){
  if(input.waypoints){waypoints.confirm();return;}
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
 const visible=input.waypoints&&draft?{...draft,pattern:waypoints.closed?'closedOneWay' as const:draft.pattern,start:waypoints.controls[0]?.anchor,end:waypoints.controls.length>1?waypoints.controls.at(-1)?.anchor:undefined,preview:waypoints.preview}:draft;
 return {draft:visible,name,setName,autoStops,setAutoStops,respectOneway,setRespectOneway,showAuto:!!input.autoStopsAvailable&&!!input.waypoints&&newRoute,error:error||waypoints.error,waiting:waiting||waypoints.checking||waypoints.committing,deleting,selectDeleting,start,cancel,pick,commit,remove,
  mutating:()=>mutation.current||waypoints.committing,rectangle:!!input.observe,waypoints:!!input.waypoints,
  extending:waypoints.extending&&!waypoints.closed,closed:waypoints.closed,closedCapable:!!input.closedCapable&&newRoute,exitExtension:waypoints.exitExtension,offerHover:waypoints.offerHover,clearHover:waypoints.clearHover,hoverPreview:waypoints.hoverPreview,allowFinish:waypoints.ready,controls:waypoints.controls,activeControl:waypoints.active==='start'?waypoints.controls[0]?.id:waypoints.controls.at(-1)?.id,
  failedLeg:waypoints.failedLeg,refusedControls:waypoints.frame?.refusal?.attempt.controls,
  history:waypoints.history,canUndo:!!(waypoints.frame?.past.length||waypoints.frame?.check||waypoints.frame?.refusal),canRedo:!!waypoints.frame?.future.length,
  canPlace:!waypoints.committing};
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
   {!draft&&!state.deleting&&(!route||multiple)&&<>
    <label><input type="checkbox" style={{width:"auto"}} checked={state.respectOneway} disabled={disabled}
     onChange={event=>state.setRespectOneway(event.target.checked)}/>地図データの一方通行に従う</label>
    <button type="button" disabled={disabled} onClick={()=>state.start('new')}>新しいバス系統</button>
   </>}
   {route&&(routes.length>1?<label>表示するバス系統<select value={route.id} disabled={disabled||!!draft||state.deleting}
    onChange={e=>onSelect(e.target.value)}>{routes.map(r=><option key={r.id} value={r.id}>{r.name}</option>)}</select></label>:<p>{route.name}</p>)}
   {route&&!draft&&!state.deleting&&<>
    <button type="button" disabled={disabled} onClick={()=>state.start('edit')}>バス系統を引き直す</button>
    <button type="button" disabled={disabled} onClick={()=>state.selectDeleting(true)}>系統を削除</button>
   </>}
   {draft&&<>
    <label>バス系統名<input maxLength={120} autoComplete="off" value={state.name} disabled={disabled} onChange={e=>state.setName(e.target.value)}/></label>
    {draft.respectOneway&&<p>地図データの一方通行に従います</p>}
    {state.showAuto&&<label>停留所の配置<select value={state.autoStops?'auto':'manual'} disabled={disabled} onChange={event=>state.setAutoStops(event.target.value==='auto')}>
     <option value="auto">自動（およそ2km間隔）</option><option value="manual">手動（起点・終点のみ）</option>
    </select></label>}
    <p role="status">{waiting?'道路経路を確認中':draft.preview?`道路経路 ${Math.round(draft.preview.lengthM)} m・${state.showAuto&&state.autoStops?'途中停留所は確定時に配置':`停留所${(state.closed?1:2)+(draft.preview.interiorStops?.length??0)+(draft.preview.loopFallbackStop?1:0)}か所`}`:draft.start?(state.waypoints?'道路上の次の点を選択':'道路上の終点を選択'):'道路上の起点を選択'}</p>
    {draft.preview?.loopFallbackStop&&<p>{draft.preview.loopFallbackStop.name}（自動）</p>}
    {draft.preview?.returnPath&&<p>復路 {Math.round(draft.preview.returnPath.lengthM)} m</p>}
    {draft.preview?.returnUnavailableReason&&<p role="alert">復路を作れません。{draft.preview.returnUnavailableReason}</p>}
    <p>{state.rectangle?"近い範囲の道路を経由します。遠い場合や範囲内に確認できない道路がある場合は選び直してください。":"この版では同じ取得範囲で起点と終点を選びます。"}停留所は両端に作られます。</p>
    {state.waypoints&&<><p>{state.closed?'環状系統（片方向）。線や点をドラッグして編集できます。':state.extending?
      (state.closedCapable?'道路を順にクリック。起点をクリックすると環状に、現在の端点かEnterで確定。':'道路を順にクリック。現在の端点をもう一度クリックするか、Enterで確定。'):
      '線や点をドラッグして編集。端点をクリックすると延長できます。'}</p>
     <button type="button" onClick={()=>state.history('undo')} disabled={!state.canUndo||state.mutating()}>点の変更を取り消す</button>
     <button type="button" onClick={()=>state.history('redo')} disabled={!state.canRedo||state.mutating()}>点の変更をやり直す</button></>}
    {draft.preview&&<button type="button" onClick={()=>void state.commit()} disabled={disabled||state.waypoints&&!state.allowFinish||!state.name.trim()||draft.pattern==="openRoundTrip"&&!draft.preview.returnPath}>バス系統を確定</button>}
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
