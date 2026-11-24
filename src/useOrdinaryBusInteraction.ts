import {useState,useRef,useEffect} from 'react';
import {appendBusStopSuggestions,verifyBusStopPlacement} from './ordinaryBusStops';
import {createOrdinaryBusHover} from './ordinaryBusHover';
import type {GeoPoint} from './alignment';
import type {Camera} from './map';
import type {RoadSnapshot} from './automaticRoadData';
import {cachedBusHoverQuery,hasObservedOrdinaryBusTile,matchOrdinaryBusPreview,prepareOrdinaryBusQuery,restoreOrdinaryBusControls,sameOrdinaryBusGeometry,sameOrdinaryBusPath,snapOrdinaryBusFreeEnd,snapOrdinaryBusVertex,type BusAnchor,type BusEvidenceReader,type OrdinaryBusGradePolicy,type OrdinaryBusPattern,type OrdinaryBusPreview,type OrdinaryBusPreviewQuery,type OrdinaryBusPreviewReply,type OrdinaryBusQuery,type RouteTileObserver} from './ordinaryBusDraft';
import {clickBusControl,closeBusCircuit,moveBusControl,insertBusControl,removeBusControl,selectBusEnd,emptyBusInteraction,undoBusDraft,redoBusDraft,requestBusFinish,settleBusValidation,type DraftControl,type OrdinaryBusInteraction} from './ordinaryBusInteraction';

type Verified={preview:OrdinaryBusPreview;token:string;path:OrdinaryBusQuery['path'];retained?:boolean};
type DraftSpec={id:string;respectOneway:boolean;pattern?:OrdinaryBusPattern;gradePolicy?:OrdinaryBusGradePolicy;savedPath?:OrdinaryBusQuery['path']};
type Input={
 active:boolean;allow:boolean;autoRoute:boolean;closedCapable?:boolean;scope:unknown;snapshot?:RoadSnapshot;loading:boolean;descriptors:readonly unknown[];
 observe?:RouteTileObserver;read:BusEvidenceReader;token:()=>Promise<string>;
 preview:(query:OrdinaryBusPreviewQuery)=>Promise<OrdinaryBusPreviewReply>;commit:(query:OrdinaryBusQuery)=>Promise<unknown>;
};
const errorText=(e:unknown)=>e instanceof Error?e.message:typeof e==='string'?e:'バス系統を変更できませんでした。';
type Frame=OrdinaryBusInteraction<Verified>;
type FreeEnd={id:string;alternatives:BusAnchor[]};
export function useOrdinaryBusInteraction(input:Input,name:string,onCommitted:()=>void){
 const [frame,render]=useState<Frame>();const frameRef=useRef<Frame|undefined>(undefined);
 const spec=useRef<DraftSpec|undefined>(undefined);const latest=useRef({input,name,onCommitted});latest.current={input,name,onCommitted};
 const previewJob=useRef<AbortController|undefined>(undefined);const running=useRef(false);
 const serialNative=useRef<Promise<unknown>>(Promise.resolve());
 const lastProof=useRef<OrdinaryBusQuery|undefined>(undefined);
 const hoverTimer=useRef<ReturnType<typeof setTimeout>|undefined>(undefined);
 const hoverPoint=useRef<{at:GeoPoint;camera:Camera;scale:number}|undefined>(undefined);
 const [hoverPreview,showHover]=useState<OrdinaryBusPreview>();
 const [hover]=useState(()=>createOrdinaryBusHover(async (work:{query:OrdinaryBusQuery;controls:readonly DraftControl[]},signal)=>{
  const {query,controls}=work;
  const request=serialNative.current.then(()=>{
   if(signal.aborted||!canHover())throw Error('preview retired');
   return latest.current.input.preview(query);
  });
  serialNative.current=request.catch(()=>undefined);
  const reply=await request;
  const matched=matchOrdinaryBusPreview(reply,controls[0].anchor,controls.at(-1)!.anchor,query.path.pattern,controls.map(c=>c.anchor));
  if(!('preview' in matched)||matched.preview.returnUnavailableReason)throw Error('no hover route');
  return matched.preview;
 },result=>showHover(result??undefined)));
 function clearHover(invalidate=false){
  hoverPoint.current=undefined;
  if(hoverTimer.current!==undefined){clearTimeout(hoverTimer.current);hoverTimer.current=undefined;}
  if(invalidate)hover.invalidate();else hover.offer(null);
 }
 function canHover(){
  return latest.current.input.active&&latest.current.input.allow&&extend.current&&!previewJob.current&&!running.current;
 }
 function offerHover(at:GeoPoint,camera:Camera,scale:number){
  if(!canHover()||!lastProof.current||!frameRef.current?.frame.verified){clearHover();return;}
  hoverPoint.current={at,camera,scale};
  if(hoverTimer.current!==undefined)return;
  hoverTimer.current=setTimeout(()=>{
   hoverTimer.current=undefined;
   const pt=hoverPoint.current,state=frameRef.current,proof=lastProof.current,src=latest.current.input;
   if(!pt||!state||!proof||state.frame.body.closed||!canHover()){hover.offer(null);return;}
   try{
    const anchor=snapOrdinaryBusVertex(src.snapshot,pt.at,pt.camera,pt.scale,undefined,true);
    const base=state.frame.body.controls;
    const placeholder={id:'b-hover-candidate',anchor};
    if(base.some(c=>c.id===placeholder.id)){hover.offer(null);return;}
    const controls=state.frame.body.active==='start'?[placeholder,...base]:[...base,placeholder];
    const query=cachedBusHoverQuery(proof,controls);
    hover.offer(query?{query:{query,controls},key:query.expectedWorldToken+':'+JSON.stringify(query.path)}:null);
   }catch{hover.offer(null);}
  },80);
 }
 const [committing,setCommitting]=useState(false);
 const [rehydrating,setRehydrating]=useState(false);
 const [extension,setExtension]=useState(false);const extend=useRef(false);
 function setExtend(value:boolean){extend.current=value;setExtension(value);}
 const [error,setError]=useState('');
 const publish=(next:Frame|undefined)=>{frameRef.current=next;render(next);};
 function reset(){
  previewJob.current?.abort();previewJob.current=undefined;spec.current=undefined;lastProof.current=undefined;clearHover(true);publish(undefined);setError('');setRehydrating(false);setExtend(false);
 }
 useEffect(()=>{reset();return ()=>{previewJob.current?.abort();previewJob.current=undefined;clearHover(true);};},[input.active,input.scope]);
 function start(value:DraftSpec){
  reset();spec.current=value;
  const fresh=emptyBusInteraction<Verified>(input.scope,32);
  if(!value.savedPath){publish(fresh);setExtend(true);return;}
  const ctrl=new AbortController(),saved=value.savedPath;
  const alive=()=>!ctrl.signal.aborted&&previewJob.current===ctrl&&latest.current.input.active&&latest.current.input.scope===input.scope;
  previewJob.current=ctrl;setRehydrating(true);
  void (async()=>{
   try{
    const token=await latest.current.input.token();if(!alive())return;
    const args:OrdinaryBusQuery={expectedWorldToken:token,routeId:value.id,name:latest.current.name.trim()||'バス系統',path:saved,rawEvidence:null,tileBinding:null};
    const request=serialNative.current.then(()=>alive()?latest.current.input.preview(args):undefined);
    serialNative.current=request.catch(()=>undefined);
    const result=await request;if(!alive()||!result)return;
    const {controls,preview}=restoreOrdinaryBusControls(saved,result);
    if(saved.pattern==='closedOneWay'&&!!preview.loopFallbackStop===!!saved.interiorStops?.length)
     throw Error('環状系統の途中停留所を確認できません。');
    if(preview.returnUnavailableReason)throw Error('既存系統の復路を確認できません。');
    lastProof.current=args;
    publish({...fresh,frame:{body:{controls,active:'end',...(saved.pattern==='closedOneWay'?{closed:true as const}:{})},verified:{token,path:saved,preview,retained:true}}});
   }catch(e){if(alive())setError(errorText(e));}
   finally{if(previewJob.current===ctrl){previewJob.current=undefined;setRehydrating(false);}}
  })();
 }
 const canEdit=()=>latest.current.input.active&&latest.current.input.allow&&!running.current;
 async function build(controls:readonly DraftControl[],currentSpec:DraftSpec,name:string,signal:AbortSignal,final=false,closed=false){
  const src=latest.current.input;
  const pattern=closed?'closedOneWay':currentSpec.pattern;
  const prior=lastProof.current;
  const cached=prior?.routeId===currentSpec.id&&prior.path.pattern===pattern&&prior.path.gradePolicy===currentSpec.gradePolicy&&prior.path.respectOneway===currentSpec.respectOneway&&cachedBusHoverQuery(prior,controls);
  if(cached&&!final)return {...cached,name};
  const expectedWorldToken=await src.token();
  if(signal.aborted)throw Error('経路の確認を中止しました。');
  if(cached&&cached.expectedWorldToken===expectedWorldToken)return {...cached,name};
  return prepareOrdinaryBusQuery(controls[0].anchor,controls.at(-1)!.anchor,name,currentSpec.id,expectedWorldToken,
   src.read,src.descriptors,signal,src.observe,pattern,controls.slice(1,-1),currentSpec.respectOneway,currentSpec.savedPath?.interiorStops,currentSpec.gradePolicy);
 }
 async function finish(f:NonNullable<Frame['finish']>){
  if(!canEdit()||!spec.current||frameRef.current?.finish!==f)return;
  if(f.verified.preview.returnUnavailableReason){
   publish({...frameRef.current,finish:undefined});
   setError(`復路を作れません。${f.verified.preview.returnUnavailableReason}`);return;
  }
  const currentSpec=spec.current,scope=input.scope;
  const controller=new AbortController();
  previewJob.current=controller;running.current=true;setCommitting(true);setError('');
  try{
   // Do not retain repeated base64 blobs in Undo history. Reacquire the
   // exact evidence and require the previously approved path/token.
   const name=latest.current.name.trim();
   const query:OrdinaryBusQuery=f.verified.retained?
    {expectedWorldToken:await latest.current.input.token(),routeId:currentSpec.id,name,path:f.verified.path,rawEvidence:null,tileBinding:null}:
    await build(f.body.controls,currentSpec,name,controller.signal,true,!!f.body.closed);
   if(controller.signal.aborted||!latest.current.input.active||latest.current.input.scope!==scope||!latest.current.input.allow||frameRef.current?.finish!==f)return;
   if(query.expectedWorldToken!==f.verified.token||!sameOrdinaryBusPath(query.path,f.verified.path))throw Error('ワールドや道路が更新されました。位置を選び直してください。');
   const title=latest.current.name.trim();
   if(!title)throw Error('バス系統名を入力してください。');
   let path=query.path;
   const current=()=>!controller.signal.aborted&&latest.current.input.active&&latest.current.input.scope===scope&&latest.current.input.allow&&frameRef.current?.finish===f;
   if(latest.current.input.autoRoute&&!currentSpec.savedPath){
    const evidence={...query,name:title,requestStopTargets:true as const};
    const result=await latest.current.input.preview({...evidence,requestAutoStopSuggestions:true});
    if(!current())return;
    const verified=verifyBusStopPlacement(path,result);
    if(!sameOrdinaryBusGeometry(f.verified.preview,verified))throw Error('停留所を確認中にバス経路が変わりました。位置を選び直してください。');
    path=appendBusStopSuggestions(path,verified,verified.autoStopSuggestions??[]);
    if(path!==query.path){
     const check=await latest.current.input.preview({...evidence,path});
     if(!current())return;
     if(!sameOrdinaryBusGeometry(verified,verifyBusStopPlacement(path,check)))throw Error('停留所を確認中にバス経路が変わりました。位置を選び直してください。');
    }
   }
   if(!current())return;
   await latest.current.input.commit({...query,name:title,path});latest.current.onCommitted();
  }catch(e){
   const now=frameRef.current;
   if(now?.finish===f&&!controller.signal.aborted){publish({...now,finish:undefined});setError(errorText(e));}
  }finally{
   if(previewJob.current===controller)previewJob.current=undefined;
   running.current=false;setCommitting(false);
  }
 }
 function handle(next:Frame,freeEnd?:FreeEnd){
  const before=frameRef.current;
  if(before===next)return;
  clearHover(true);publish(next);setError('');
  if(next.finish){void finish(next.finish);return;}
  if(before?.check?.id===next.check?.id)return; // only finish intent/active endpoint changed.
  previewJob.current?.abort();previewJob.current=undefined;
  const check=next.check,currentSpec=spec.current;
  if(!check||!currentSpec)return;
  const ctrl=new AbortController();previewJob.current=ctrl;
  const alive=()=>!ctrl.signal.aborted&&previewJob.current===ctrl&&latest.current.input.active&&latest.current.input.scope===next.scope&&frameRef.current?.check?.id===check.id;
  void (async()=>{
   try{
    const title=latest.current.name.trim();
    // The name field can remain temporarily empty while laying out geometry.
    let query=await build(check.body.controls,currentSpec,title||'バス系統',ctrl.signal,false,!!check.body.closed);
    if(!alive())return;
    // At most one native graph query in flight per editor. Retired
    // candidates are not allowed to queue another request.
    const queryNative=(job:OrdinaryBusQuery,valid=alive)=>{
     const request=serialNative.current.then(()=>valid()?latest.current.input.preview(job):undefined);
     serialNative.current=request.catch(()=>undefined);
     return request;
    };
    const match=(reply:OrdinaryBusPreviewReply,controls:readonly DraftControl[],pattern?:OrdinaryBusPattern)=>
     matchOrdinaryBusPreview(reply,controls[0].anchor,controls.at(-1)!.anchor,pattern,controls.map(c=>c.anchor));
    const controls=check.body.controls;
    const reply=await queryNative(query);
    if(!alive()||!reply)return;
    let matched=match(reply,controls,query.path.pattern);
    let resolvedEnd:DraftControl|undefined;
    const freeIndex=freeEnd?controls.findIndex(c=>c.id===freeEnd.id):-1;
    if(freeEnd&&!check.body.closed&&(freeIndex===0||freeIndex===controls.length-1)&&
       'refusedLeg' in matched&&matched.refusedLeg===Math.max(0,freeIndex-1)){
     for(const anchor of freeEnd.alternatives){
      const current=()=>alive()&&latest.current.input.allow&&hasObservedOrdinaryBusTile(latest.current.input.snapshot,anchor.tile);
      if(!current())break;
      if(controls.some(c=>c.anchor.point.latitude===anchor.point.latitude&&c.anchor.point.longitude===anchor.point.longitude))continue;
      const moved={id:freeEnd.id,anchor};
      const candidate=controls.map(c=>c.id===moved.id?moved:c);
      const nextQuery=cachedBusHoverQuery(query,candidate);
      if(!nextQuery)continue;
      const alt=await queryNative(nextQuery,current);
      if(!current()||!alt)break;
      const checked=match(alt,candidate,nextQuery.path.pattern);
      if('preview' in checked&&!checked.preview.returnUnavailableReason){
       matched=checked;query=nextQuery;resolvedEnd=moved;break;
      }
     }
    }
    if('preview' in matched&&query.path.pattern==='closedOneWay'&&!!matched.preview.loopFallbackStop===!!query.path.interiorStops?.length)
     throw Error('環状系統の途中停留所を確認できません。');
    const outcome:{reason:string;leg?:number}|{verified:Verified;resolvedEnd?:DraftControl}='refusedLeg' in matched?{reason:'この区間を道路でつなげません。',leg:matched.refusedLeg}:
      matched.preview.returnUnavailableReason&&!(latest.current.input.closedCapable&&!currentSpec.savedPath)?{reason:`復路を作れません。${matched.preview.returnUnavailableReason}`}:
      {verified:{preview:matched.preview,token:query.expectedWorldToken,path:query.path},resolvedEnd};
    if('verified' in outcome)lastProof.current=query;
    handle(settleBusValidation<Verified>(frameRef.current!,next.scope,check.id,outcome));
   }catch(e){
    if(alive())handle(settleBusValidation<Verified>(frameRef.current!,next.scope,check.id,{reason:errorText(e)}));
   }finally{if(previewJob.current===ctrl)previewJob.current=undefined;}
  })();
 }
 function point(at:GeoPoint,camera:Camera,scale:number,detail:number,handleId?:string,moveId?:string,afterId?:string){
  const now=frameRef.current;if(!canEdit()||!now)return;
  const body=now.check?.body??now.frame.body,controls=body.controls;
  if(detail>=2&&!moveId&&!afterId){if(extend.current&&!body.closed)handle(requestBusFinish(now));return;}
  const handleAt=controls.find(c=>c.id===handleId);
  if(body.closed&&!moveId&&!afterId){
   if(handleAt&&handleAt.id!==controls[0].id&&handleAt.id!==controls.at(-1)?.id)handle(removeBusControl(now,handleAt.id));
   return;
  }
  if(extend.current&&!spec.current?.savedPath&&latest.current.input.closedCapable&&handleAt&&handleAt===controls[0]&&body.active==='end'&&controls.length>=2&&!moveId&&!afterId){
   handle(closeBusCircuit(now,handleAt.id,`b-${crypto.randomUUID()}`));return;
  }
  if(!extend.current&&!moveId&&!afterId){
   if(!handleAt)return; // blank clicks during edit never append.
   const index=controls.indexOf(handleAt);
   if(index===0||index===controls.length-1){
    setExtend(true);handle(selectBusEnd(now,index===0?'start':'end'));return;
   }
   handle(removeBusControl(now,handleAt.id));return;
  }
  try{
   if(!handleAt&&!latest.current.input.snapshot)throw Error(latest.current.input.loading?'道路を読み込み中です。':'道路を確認できませんでした。地図を拡大して選び直してください。');
   const snapshot=latest.current.input.snapshot,free=extend.current&&controls.length>0&&!moveId&&!afterId&&!handleAt;
   const [anchor,...alternatives]=handleAt?[handleAt.anchor]:free?snapOrdinaryBusFreeEnd(snapshot,at,camera,scale,body.active==='end'?controls.map(c=>c.anchor):[]):
    [snapOrdinaryBusVertex(snapshot,at,camera,scale,undefined,true)];
   if(moveId){handle(moveBusControl(now,moveId,anchor));return;}
   if(afterId){handle(insertBusControl(now,afterId,{id:`b-${crypto.randomUUID()}`,anchor}));return;}
   const picked=handleAt??{id:`b-${crypto.randomUUID()}`,anchor};
   handle(clickBusControl(now,picked,detail),alternatives.length?{id:picked.id,alternatives}:undefined);
  }catch(e){setError(errorText(e));}
 }
 function history(direction:'undo'|'redo'){
  const now=frameRef.current;if(!canEdit()||!now)return;
  handle(direction==='undo'?undoBusDraft(now):redoBusDraft(now));
 }
 function confirm(){
  const now=frameRef.current;if(canEdit()&&now)handle(requestBusFinish(now));
 }
 function exitExtension(){
  if(!spec.current?.savedPath||!extend.current||running.current)return false;
  const now=frameRef.current;
  if(now&&(now.check||now.refusal||now.finish))handle(undoBusDraft(now));
  clearHover(true);setExtend(false);return true;
 }
 // Do not expose previous valid preview as savable while a new pick is under review.
 const controls=(frame?.check?.body??frame?.frame.body)?.controls??[];
 const active=(frame?.check?.body??frame?.frame.body)?.active??'end';
 return {frame,controls,active,start,reset,point,confirm,history,closed:!!(frame?.check?.body??frame?.frame.body)?.closed,committing,offerHover,clearHover,hoverPreview,exitExtension,extending:extension,
  error:error||frame?.refusal?.reason||'',failedLeg:frame?.refusal?.leg,checking:rehydrating||!!frame?.check,
  ready:!!frame?.frame.verified&&!frame.check&&!frame.refusal&&!frame.finish,
  preview:frame?.check||frame?.refusal&&!spec.current?.savedPath?undefined:frame?.frame.verified?.preview};
}
