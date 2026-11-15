import {useEffect,useRef,useState} from 'react';
import type {ReferenceCopyState} from './useReferenceCopy';
import type {ReferenceBuild} from './referenceCopy';
import type {SourceRecord} from './AlignmentProvenance';
import type {GraphBounds,GraphPoint} from './referenceGraph';
import type {RailPick,RailRoute} from './realRailGraph';
import type {RailRequest,RailResponse,RailView} from './realRailProvider';
import {MAP_BOUNDS} from './map';
export type RailStatus='unknown'|'loading'|'ready'|'failed';
const emptyView:RailView={chains:[],truncated:false};
export function useAutomaticRail({selected,enabled,canStart,worldEpoch,name,bounds,zoom,radius,onBuild}:{selected:boolean;enabled:boolean;canStart:boolean;worldEpoch:unknown;name:string;bounds:GraphBounds;zoom:number;radius:number;onBuild:(build:ReferenceBuild)=>Promise<void>}){
 const [status,setStatus]=useState<RailStatus>('unknown'),[message,setMessage]=useState(''),[view,setView]=useState<RailView>(emptyView),[active,setActive]=useState(false),[busy,setBusy]=useState(false),[confirmed,setConfirmed]=useState(false),[picks,setPicks]=useState<RailPick[]>([]),[redo,setRedo]=useState<RailPick[]>([]),[route,setRoute]=useState<RailRoute|null>(null),[attempt,setAttempt]=useState(0);
 const worker=useRef<Worker|null>(null),source=useRef<SourceRecord|null>(null),sequence=useRef(0),pickRequest=useRef(0),viewRequest=useRef(0),pending=useRef(false),callbacks=useRef(new Map<number,(r:RailResponse)=>void>());
 const boundsKey=JSON.stringify(bounds),viewKey=useRef('');
 function request(message:Omit<Extract<RailRequest,{kind:'route'}>,'id'>|Omit<Extract<RailRequest,{kind:'pick'}>,'id'>|Omit<Extract<RailRequest,{kind:'view'}>,'id'>,callback:(r:RailResponse)=>void){const id=++sequence.current;callbacks.current.set(id,callback);worker.current?.postMessage({...message,id});return id;}
 function clear(){pickRequest.current++;pending.current=false;setBusy(false);setActive(false);setPicks([]);setRedo([]);setRoute(null);setConfirmed(false);}
 useEffect(()=>{
  clear();setView(emptyView);source.current=null;callbacks.current.clear();setMessage('');
  if(!selected){setStatus('unknown');return;}
  setStatus('loading');let disposed=false;
  try {
   const instance=new Worker(new URL('./realRail.worker.ts',import.meta.url),{type:'module'});worker.current=instance;
   instance.onmessage=(event:MessageEvent<RailResponse>)=>{if(disposed)return;const response=event.data;if(response.id===0){if(response.kind==='loaded'){source.current=response.source;setStatus('ready');setMessage('日本の参照線路網を取得しました。網羅性は保証されません。');}else if(response.kind==='error'){setStatus('failed');setMessage(response.message);}return;}const callback=callbacks.current.get(response.id);callbacks.current.delete(response.id);callback?.(response);};
   instance.onerror=()=>{if(!disposed){callbacks.current.clear();clear();setStatus('failed');setMessage('線路workerが停止しました。再取得してください。');}};
   instance.postMessage({id:0,kind:'load'} satisfies RailRequest);
  }catch(error){setStatus('failed');setMessage(String(error));}
  return()=>{disposed=true;worker.current?.terminate();worker.current=null;callbacks.current.clear();pickRequest.current++;};
 },[selected,worldEpoch,attempt]);
 useEffect(()=>{
  viewKey.current='';setView(emptyView);
  // Invalidate a click pending against an older displayed region. Confirmed
  // endpoints/route stay fixed while panning, just as ordinary planned geometry.
  pickRequest.current++;pending.current=false;setBusy(false);
  if(status!=='ready'||zoom<14)return;
  const token=++viewRequest.current;
  const timer=setTimeout(()=>{request({kind:'view',bounds},response=>{if(token!==viewRequest.current)return;if(response.kind==='view'){viewKey.current=boundsKey;setView(response.view);}else if(response.kind==='error')setMessage(response.message);});},120);
  return()=>{clearTimeout(timer);viewRequest.current++;};
 },[boundsKey,zoom,status,worldEpoch]);
 const build:ReferenceBuild|null=route&&source.current?{name:name.trim(),points:route.points,sourceRecord:source.current,provenance:{source_key:source.current.key,method:'rail_graph_route_v1',osm_way_id:'',mode:'rail',start_segment_index:0,start_fraction:0,end_segment_index:0,end_fraction:0,network_path:route.spans}}:null;
 function begin(){if(!enabled||!canStart||status!=='ready')return;clear();setActive(true);setMessage('点線から12px以内をクリックして起点・終点を選択してください。');}
 function query(point:GraphPoint,start:RailPick|null,onResult:(pick:RailPick,route:RailRoute|null)=>void){
  if(!enabled||!active||pending.current||status!=='ready')return;
  if(zoom<14||view.truncated||viewKey.current!==boundsKey){setMessage('地図を拡大し、点線の表示を待ってください。');return;}
  const token=++pickRequest.current;pending.current=true;setBusy(true);setConfirmed(false);setRoute(null);
  request({kind:'pick',point,radius,visibleIds:view.chains.map(c=>c.id),start},response=>{
   if(token!==pickRequest.current)return;pending.current=false;setBusy(false);
   if(response.kind==='error'){setMessage(response.message);return;}
   if(response.kind!=='pick'||!response.pick){setMessage('表示中の参照線路が12px以内にありません。実在線路の不存在は意味しません。');return;}
   if(response.route?.points.some(p=>p.longitude<MAP_BOUNDS.west||p.longitude>MAP_BOUNDS.east||p.latitude<MAP_BOUNDS.south||p.latitude>MAP_BOUNDS.north)){setMessage('経路が建設範囲外です');return;}
   onResult(response.pick,response.route);setMessage(response.route?'点線と経路・出典を確認し、計画に追加してください。':'起点を選択しました。終点をクリックしてください。');
  });
 }
 function click(point:GraphPoint){query(point,picks[0]??null,(pick,result)=>{setPicks(picks.length?[picks[0],pick]:[pick]);setRedo([]);setRoute(result);});}
 function undo(){if(!enabled||pending.current||!picks.length)return;pickRequest.current++;setConfirmed(false);setRoute(null);setRedo(old=>[...old,picks.at(-1)!]);setPicks(old=>old.slice(0,-1));}
 function redoPick(){
  if(!enabled||pending.current||!active||!redo.length||status!=='ready')return;
  const end=redo.at(-1)!;
  if(!picks.length){setPicks([end]);setRedo(old=>old.slice(0,-1));return;}
  const token=++pickRequest.current;pending.current=true;setBusy(true);setConfirmed(false);
  request({kind:'route',start:picks[0],end},response=>{
   if(token!==pickRequest.current)return;pending.current=false;setBusy(false);
   if(response.kind==='pick'&&response.route){setPicks([picks[0],end]);setRoute(response.route);setRedo(old=>old.slice(0,-1));}
   else if(response.kind==='error')setMessage(response.message);
  });
 }

 async function commit(){if(!enabled||pending.current||!active||!confirmed||!build||!name.trim())return;const token=pickRequest.current;pending.current=true;setBusy(true);try{await onBuild(build);if(token===pickRequest.current){clear();setMessage('実在形状の経路を計画に追加しました。');}}catch(error){if(token===pickRequest.current)setMessage(String(error));}finally{if(token===pickRequest.current){pending.current=false;setBusy(false);}}}
 const copy:ReferenceCopyState={active,mode:'rail',picks:picks.map(p=>({datasetId:'trainsworld-realgraph-jp',revision:source.current?.revision??'',edgeId:String(p.chain),fraction:p.position})),redo:redo.map(p=>({datasetId:'trainsworld-realgraph-jp',revision:source.current?.revision??'',edgeId:String(p.chain),fraction:p.position})),candidates:[],roadFollow:false,roadAvailable:false,hoverCandidates:[],hover:async()=>{},clearHover:()=>{},message,confirmed,setConfirmed,busy,plan:{build,error:''},begin,clear,click:async p=>click(p),choose:()=>{},undo,redoPick,build:commit,setMode:()=>{}};
 return {copy,status,message,view,ready:status==='ready',retry:()=>setAttempt(n=>n+1)};
}
