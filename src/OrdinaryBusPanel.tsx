import type {BusInteriorStop} from './ordinaryBusDraft';
import {useEffect,useRef,useState} from 'react';
import {useOrdinaryBusInteraction} from './useOrdinaryBusInteraction';
import type {GeoPoint} from './alignment';
import type {Camera} from './map';
import type {RoadDataState,RoadSnapshot} from './automaticRoadData';
import {OFM_EVIDENCE_LIMITS,OFM_LIMITS} from './openFreeMapRoadData';
import {hasObservedOrdinaryBusTile,matchOrdinaryBusPreview,ordinaryBusTileFootprint,prepareNearbyOrdinaryBusQuery,prepareOrdinaryBusQuery,retryOrdinaryBusSharedSeam,snapOrdinaryBusSegment,snapOrdinaryBusVertex,type BusAnchor,type BusEvidenceReader,type OrdinaryBusPreview,type OrdinaryBusPreviewQuery,type OrdinaryBusPreviewReply,type OrdinaryBusPath,type OrdinaryBusQuery,type OrdinaryBusPattern,type RouteTileObserver} from './ordinaryBusDraft';

type Route={id:string;name:string;pattern?:OrdinaryBusPattern;path?:OrdinaryBusPath};
type Draft={id:string;respectOneway:boolean;keepSharedSeam?:boolean;fractional?:true;generalFractional?:true;savedInteriorStops?:BusInteriorStop[];pattern?:OrdinaryBusPattern;start?:BusAnchor;end?:BusAnchor;preview?:OrdinaryBusPreview;query?:OrdinaryBusQuery};
type Input={
 on:boolean;allow:boolean;sharedSeam?:boolean;scope:unknown;snapshot?:RoadSnapshot;loading:boolean;
 route?:Route;multiple:boolean;roundTrip:boolean;fractionalAvailable?:boolean;generalFractionalAvailable?:boolean;waypoints?:boolean;autoStopsAvailable?:boolean;closedCapable?:boolean;observedGrade?:boolean;conflictingGraph:boolean;descriptors:readonly unknown[];
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
 const [newPattern,setNewPattern]=useState<'roundTrip'|'oneWay'>('roundTrip');
 const [name,setName]=useState('バス系統');
 const [error,setError]=useState('');
 const [waiting,setWaiting]=useState(false);
 const [deleting,setDeleting]=useState(false);
 const job=useRef<AbortController|undefined>(undefined);
 const mutation=useRef(false);
 const current=useRef(input);
 current.current=input;
 const usingWaypoints=!!input.waypoints&&!draft?.fractional;
 const fractionalReady=(start:BusAnchor)=>current.current.fractionalAvailable&&hasObservedOrdinaryBusTile(current.current.snapshot,start.tile);
 const waypoints=useOrdinaryBusInteraction({...input,active:input.on&&usingWaypoints,autoRoute:!!input.autoStopsAvailable&&autoStops&&newRoute},name,()=>cancel());
 function cancel(){
  waypoints.reset();
  job.current?.abort();job.current=undefined;setWaiting(false);setDraft(undefined);setError('');setDeleting(false);
 }
 useEffect(()=>{
  cancel();setRespectOneway(false);setNewPattern('roundTrip');
  return ()=>{job.current?.abort();job.current=undefined;};
 },[input.on,input.scope]);
 function start(kind:'new'|'edit'|'fractional'){
  if(!input.on||!input.allow||input.conflictingGraph||mutation.current)return;
  const editing=kind==='edit'?input.route:undefined;
  if((kind==='edit'&&!editing)||(kind!=='edit'&&input.route&&!input.multiple))return;
  const generalFractional=!!editing?.path?.fractionalTerminals||kind==='fractional'&&!!input.generalFractionalAvailable;
  const fractional=generalFractional||kind==='fractional'||!!editing?.path?.sameAtomStops;
  if(fractional&&(!input.fractionalAvailable||generalFractional&&!input.generalFractionalAvailable||editing?.pattern))return;
  const pattern=fractional?undefined:editing?editing.pattern:input.roundTrip&&newPattern==='roundTrip'?'openRoundTrip':undefined;
  cancel();const spec={id:editing?.id??`bus:${crypto.randomUUID()}`,pattern,
   ...(fractional?{fractional:true as const}:{}),
   ...(generalFractional?{generalFractional:true as const,savedInteriorStops:editing?.path?.interiorStops}:{}),
   keepSharedSeam:editing?.path?.rule==='ofm_ground_bus_shared_seam_z14_v3',
   respectOneway:editing?editing.path?.respectOneway??false:respectOneway};setDraft(spec);
  if(input.waypoints&&!fractional)waypoints.start({...spec,savedPath:editing?.path,
   gradePolicy:editing?editing.path?.gradePolicy:input.observedGrade?'observedConnectors':undefined});
  setNewRoute(!editing);setAutoStops(!editing&&!!input.autoStopsAvailable);
  setName(editing?.name??'バス系統');
 }
 async function pick(at:GeoPoint,camera:Camera,scale:number,detail=1,handleId?:string,moveId?:string,afterId?:string){
  if(usingWaypoints){waypoints.point(at,camera,scale,detail,handleId,moveId,afterId);return;}
  if(!input.on||!input.allow||!draft||draft.preview||job.current||mutation.current||
     draft.fractional&&(detail>1||!input.fractionalAvailable))return;
  setError('');
  if(!input.snapshot){
   setError(input.loading?'道路を読み込み中です。':'道路を確認できませんでした。地図を拡大して選び直してください。');return;
  }
  let end:BusAnchor;
  try{end=draft.fractional?snapOrdinaryBusSegment(input.snapshot,at,camera,scale):
   snapOrdinaryBusVertex(input.snapshot,at,camera,scale,draft.start,!!input.observe);}
  catch(e){setError(displayError(e));return;}
  if(!draft.start){setDraft({...draft,start:end});return;}
  const start=draft.start;
  const title=name.trim();
  if(!title){setError('バス系統名を入力してください。');return;}
  const controller=new AbortController();
  const alive=()=>!controller.signal.aborted&&job.current===controller&&
   current.current.on&&current.current.scope===input.scope;
  const verifiedSource=()=>{
   if(draft.fractional&&(!fractionalReady(start)||draft.generalFractional&&!fractionalReady(end)))throw Error('道路の元データが変わりました。起点から選び直してください。');
  };
  job.current=controller;setWaiting(true);
  try{
   verifiedSource();
   const token=await input.token();
   if(!alive())return;
   verifiedSource();
   let query=await prepareOrdinaryBusQuery(start,end,title,draft.id,token,input.read,input.descriptors,controller.signal,input.observe,draft.pattern,[],draft.respectOneway,draft.savedInteriorStops??[],undefined,!!draft.generalFractional);
   if(!alive())return;
   verifiedSource();
   if(draft.keepSharedSeam)query=retryOrdinaryBusSharedSeam(query)??query;
   const result=await input.preview(query);
   if(!alive())return;
   verifiedSource();
   let matched=matchOrdinaryBusPreview(result,start,end,draft.pattern,[start,end]);
   if('refusedLeg' in matched&&newRoute&&!draft.fractional&&current.current.sharedSeam&&current.current.allow){
    const retry=retryOrdinaryBusSharedSeam(query);
    if(retry){
     const alt=await current.current.preview(retry);
     if(!alive())return;
     const checked=matchOrdinaryBusPreview(alt,start,end,draft.pattern,[start,end]);
     if('preview' in checked&&!checked.preview.returnUnavailableReason){matched=checked;query=retry;}
    }
   }
   if('refusedLeg' in matched&&newRoute&&!draft.fractional&&current.current.observe&&current.current.allow){
    const src=current.current;
    let extra=await prepareNearbyOrdinaryBusQuery(query,src.snapshot,src.read,src.descriptors,controller.signal);
    if(!alive())return;
    if(extra&&current.current.allow){
     const alt=await current.current.preview(extra);
     if(!alive()||!current.current.allow)return;
     let checked=matchOrdinaryBusPreview(alt,start,end,draft.pattern,[start,end]);
     const retry='refusedLeg' in checked&&current.current.sharedSeam?retryOrdinaryBusSharedSeam(extra):undefined;
     if(retry){
      const revised=await current.current.preview(retry);
      if(!alive()||!current.current.allow)return;
      checked=matchOrdinaryBusPreview(revised,start,end,draft.pattern,[start,end]);extra=retry;
     }
     if('preview' in checked&&!checked.preview.returnUnavailableReason){matched=checked;query=extra;}
    }
   }
   if('refusedLeg' in matched)throw Error('この区間を道路でつなげません。');
   const preview=matched.preview;
   if(draft.fractional&&!draft.generalFractional&&preview.points.length!==2)throw Error('道路区間の確認結果が一致しません。');
   if(alive())setDraft({...draft,end,preview,query});
  }catch(e){
   if(alive())setError(displayError(e));
  }finally{
   if(job.current===controller){job.current=undefined;setWaiting(false);}
  }
 }
 async function commit(){
  if(usingWaypoints){waypoints.confirm();return;}
  if(!input.on||!input.allow||!draft?.query||draft.pattern==="openRoundTrip"&&!draft.preview?.returnPath||waiting||mutation.current||!name.trim())return;
  if(draft.fractional&&(!draft.start||!fractionalReady(draft.start)||
     draft.generalFractional&&(!draft.end||!fractionalReady(draft.end)||!input.generalFractionalAvailable))){
   setDraft({...draft,end:undefined,preview:undefined,query:undefined,start:undefined});
   setError('道路の元データが変わりました。起点から選び直してください。');return;
  }
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
 const visible=usingWaypoints&&draft?{...draft,pattern:waypoints.closed?'closedOneWay' as const:draft.pattern,start:waypoints.controls[0]?.anchor,end:waypoints.controls.length>1?waypoints.controls.at(-1)?.anchor:undefined,preview:waypoints.preview}:draft;
 return {draft:visible,name,setName,autoStops,setAutoStops,respectOneway,setRespectOneway,showAuto:!!input.autoStopsAvailable&&usingWaypoints&&newRoute,chooseNewPattern:input.roundTrip,newPattern,setNewPattern,error:error||waypoints.error,waiting:waiting||waypoints.checking||waypoints.committing,deleting,selectDeleting,start,cancel,pick,commit,remove,
  mutating:()=>mutation.current||waypoints.committing,rectangle:!!input.observe&&!draft?.fractional,waypoints:usingWaypoints,fractionalAvailable:!!input.fractionalAvailable,
  extending:waypoints.extending&&!waypoints.closed,closed:waypoints.closed,closedCapable:!!input.closedCapable&&newRoute,exitExtension:waypoints.exitExtension,offerHover:waypoints.offerHover,clearHover:waypoints.clearHover,hoverPreview:waypoints.hoverPreview,allowFinish:waypoints.ready,controls:waypoints.controls,activeControl:waypoints.active==='start'?waypoints.controls[0]?.id:waypoints.controls.at(-1)?.id,
  failedLeg:waypoints.failedLeg,refusedControls:waypoints.frame?.refusal?.attempt.controls,
  history:waypoints.history,shorten:waypoints.shorten,canShorten:waypoints.canShorten,canUndo:!!(waypoints.frame?.past.length||waypoints.frame?.check||waypoints.frame?.refusal),canRedo:!!waypoints.frame?.future.length,
  canPlace:!waypoints.committing};
}

export function OrdinaryBusPanel({state,route,routes,onSelect,multiple,blocked,enabled,roadSource,editingLocked=false}:{
 state:ReturnType<typeof useOrdinaryBusDraft>;route?:Route;routes:readonly Route[];
 onSelect:(id:string)=>void;multiple:boolean;blocked:boolean;enabled:boolean;editingLocked?:boolean;
 roadSource:Pick<RoadDataState,'status'|'message'>&{retry:()=>void};
}){
 const {draft,waiting,error}=state,disabled=!enabled||waiting;
 const oneWayGuidance=state.chooseNewPattern&&(!route||multiple)
  ?'片道で運行する場合は下書きを取り消し、新しい系統を「片道」で作成してください。':'';
 const tileLimit=state.rectangle?OFM_EVIDENCE_LIMITS.batchTiles:1;
 const chosen=state.waypoints?state.controls.map(({anchor})=>anchor.tile):[draft?.start?.tile,draft?.end?.tile].filter(tile=>tile!==undefined);
 let tileFootprint:number|undefined;
 if(chosen.length){
  try{tileFootprint=ordinaryBusTileFootprint(chosen[0],chosen.at(-1)!,chosen.slice(1,-1)).count;}
  catch{/* Unknown or inconsistent source: never estimate the footprint. */}
 }
 return <section aria-label="バス系統の建設">
  <h3>バス</h3>
  <p>地図の道路を使ってゲーム内の系統を作ります。実際の通行可否は示しません。</p>
  {blocked?<p>このワールドの別の道路交通データは上書きできません。</p>:<>
   {roadSource.status!=='ready'&&roadSource.message&&
    <div role="status" aria-label="バスの道路データ状態"><p>{roadSource.message}</p>
     {roadSource.status==='error'&&<button type="button" className="secondary" disabled={disabled||state.mutating()}
      onClick={roadSource.retry}>道路データを再取得</button>}
    </div>}
   <details>
    <summary>バス経路のデータ上限</summary>
    <p>選択点を囲む道路の確認範囲は最大 {tileLimit} タイル。元データ1タイル最大{OFM_LIMITS.bytes/2**20} MiB、1回の転送合計最大{OFM_EVIDENCE_LIMITS.batchBytes/2**20} MiBです。</p>
    <p>タイル数は取得済みの件数ではありません。経路が成立するか、転送上限に収まるかは別に確認します。</p>
   </details>
   {editingLocked&&<p>確認済みの境界横断バスを含むため、経路の引き直しはできません。</p>}
   {!editingLocked&&!draft&&!state.deleting&&(!route||multiple)&&<>
    {state.chooseNewPattern&&<label>新しいバス系統の運行方向<select value={state.newPattern} disabled={disabled}
     onChange={event=>state.setNewPattern(event.target.value==='oneWay'?'oneWay':'roundTrip')}>
     <option value="roundTrip">往復（往路と復路を確認）</option><option value="oneWay">片道（起点から終点のみ）</option>
    </select></label>}
    <label><input type="checkbox" style={{width:"auto"}} checked={state.respectOneway} disabled={disabled}
     onChange={event=>state.setRespectOneway(event.target.checked)}/>地図データの一方通行に従う</label>
    <button type="button" disabled={disabled} onClick={()=>state.start('new')}>新しいバス系統</button>
    {state.fractionalAvailable&&<button type="button" disabled={disabled} className="secondary"
     onClick={()=>state.start('fractional')}>道路の途中に起終点を置く（片道）</button>}
   </>}
   {route&&(routes.length>1?<label>表示するバス系統<select value={route.id} disabled={disabled||!!draft||state.deleting}
    onChange={e=>onSelect(e.target.value)}>{routes.map(r=><option key={r.id} value={r.id}>{r.name}</option>)}</select></label>:<p>{route.name}</p>)}
   {route&&!editingLocked&&!draft&&!state.deleting&&<>
    {state.chooseNewPattern&&<p>運行方向: {route.pattern==='openRoundTrip'?'往復':route.pattern==='closedOneWay'?'環状':'片道'}</p>}
    <button type="button" disabled={disabled||!!(route.path?.sameAtomStops||route.path?.fractionalTerminals)&&(!state.fractionalAvailable||!!route.pattern)}
     onClick={()=>state.start('edit')}>バス系統を引き直す</button>
    <button type="button" disabled={disabled} onClick={()=>state.selectDeleting(true)}>系統を削除</button>
   </>}
   {draft&&<>
    {tileFootprint!==undefined&&<p>選択点から必要な確認範囲: {tileFootprint} / {tileLimit} タイル</p>}
    <label>バス系統名<input maxLength={120} autoComplete="off" value={state.name} disabled={disabled} onChange={e=>state.setName(e.target.value)}/></label>
    {draft.respectOneway&&<p>地図データの一方通行に従います</p>}
    {!draft.fractional&&state.chooseNewPattern&&!state.closed&&<p>運行方向: {draft.pattern==='openRoundTrip'?'往復':'片道'}</p>}
    {draft.fractional&&<p>{draft.generalFractional?"確認できる道路を経由する片道系統です。復路は追加できません。":"同じ道路の一部分だけを使う片道系統です。交差点、途中停留所、複数の地図区画をまたぐ併用軌道・時間帯ダイヤとの併用はこの版では対応しません。"}</p>}
    {state.showAuto&&<label>停留所の配置<select value={state.autoStops?'auto':'manual'} disabled={disabled} onChange={event=>state.setAutoStops(event.target.value==='auto')}>
     <option value="auto">自動（およそ2km間隔）</option><option value="manual">手動（起点・終点のみ）</option>
    </select></label>}
    <p role="status">{waiting?'道路経路を確認中':draft.preview?`道路経路 ${draft.fractional?Number(draft.preview.lengthM.toFixed(3)):Math.round(draft.preview.lengthM)} m・${state.showAuto&&state.autoStops?'途中停留所は確定時に配置':`停留所${(state.closed?1:2)+(draft.preview.interiorStops?.length??0)+(draft.preview.loopFallbackStop?1:0)}か所`}`:draft.start?(state.waypoints?'道路上の次の点を選択':'道路上の終点を選択'):'道路上の起点を選択'}</p>
    {draft.preview?.loopFallbackStop&&<p>{draft.preview.loopFallbackStop.name}（自動）</p>}
    {draft.preview?.returnPath&&<p>復路 {Math.round(draft.preview.returnPath.lengthM)} m</p>}
    {draft.preview?.returnUnavailableReason&&<p role="alert">復路を作れません。{draft.preview.returnUnavailableReason}
     {oneWayGuidance&&<> {oneWayGuidance}</>}
    </p>}
    {!draft.fractional&&<p>{state.rectangle?"近い範囲の道路を経由します。遠い場合や範囲内に確認できない道路がある場合は選び直してください。":"この版では同じ取得範囲で起点と終点を選びます。"}停留所は両端に作られます。</p>}
    {state.waypoints&&<><p>{state.closed?'環状系統（片方向）。線や点をドラッグして編集できます。':state.extending?
      (state.closedCapable?'道路を順にクリック。起点をクリックすると環状に、現在の端点かEnterで確定。':'道路を順にクリック。現在の端点をもう一度クリックするか、Enterで確定。'):
      '線や点をドラッグして編集。端点をクリックすると延長できます。'}</p>
     {state.canShorten&&<>
      <p>端の区間を外し、隣の経由点を新しい端点にします。</p>
      <button type="button" disabled={disabled} onClick={()=>state.shorten('start')}>起点を1点短縮</button>
      <button type="button" disabled={disabled} onClick={()=>state.shorten('end')}>終点を1点短縮</button>
     </>}
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
  {error&&<p role="alert">{error}{!draft?.preview?.returnUnavailableReason&&error.startsWith('復路を作れません。')&&oneWayGuidance&&<> {oneWayGuidance}</>}</p>}
 </section>;
}
