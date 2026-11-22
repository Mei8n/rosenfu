import './directoryPanel.css';
import {useEffect,useMemo,useRef,useState} from 'react';
import {firstByKey} from './mapLookupIndex';
import {alignmentLength,type GeoPoint} from './alignment';
import {corridorSpans} from './connectedCorridor';
import type {EndpointConnection} from './destinationJoin';
import type {Segment} from './RouteCandidatePreview';
type Route={id:string;name:string;start:string};
type World={routes:Route[];alignments?:{id:string;points:GeoPoint[]}[];station_placements?:{station_id:string;alignment_id:string;position:number}[];alignment_connections?:EndpointConnection[];route_paths?:{route_id:string;segments:Segment[]}[]};
type RouteGeometry=Omit<World,'routes'>;
function connectionKey({a,b}:EndpointConnection){
 const left=JSON.stringify([a.alignment_id,a.endpoint]),right=JSON.stringify([b.alignment_id,b.endpoint]);
 return left<right?JSON.stringify([left,right]):JSON.stringify([right,left]);
}
/** One read-only index per geometry snapshot; compute physical meters lazily. */
export function indexRouteLengths(world:RouteGeometry):(routeId:string)=>number|null {
 const paths=firstByKey(world.route_paths??[],path=>path.route_id);
 const lines=firstByKey(world.alignments??[],line=>line.id);
 const placements=firstByKey(world.station_placements??[],p=>p.station_id);
 const connections=new Set((world.alignment_connections??[]).map(connectionKey));
 const metersByLine=new Map<string,number>(),metersByRoute=new Map<string,number|null>();
 const placement=(id:string)=>placements.get(id);
 const lookup={placement,hasConnection:(link:EndpointConnection)=>connections.has(connectionKey(link))};
 function span(id:string,begin:number,finish:number){
  const line=lines.get(id);
  if(!line||!Number.isFinite(begin)||!Number.isFinite(finish)||begin<0||finish<0||begin>1||finish>1||begin===finish)throw Error('No measurable span');
  let total=metersByLine.get(id);
  if(total===undefined){total=alignmentLength(line.points);metersByLine.set(id,total);}
  const length=total*Math.abs(finish-begin);
  if(!Number.isFinite(length)||length<=0)throw Error('No measurable length');
  return length;
 }
 function between(id:string,from:string,to:string){const a=placement(from),b=placement(to);if(!a||!b||a.alignment_id!==id||b.alignment_id!==id)throw Error('Missing placement');return span(id,a.position,b.position);}
 return routeId=>{
  const existing=metersByRoute.get(routeId);
  if(existing!==undefined)return existing;
  const segments=paths.get(routeId)?.segments??[];
  let length:number|null=null;
  if(segments.length){
   try{length=segments.reduce((total,s)=>{
    if(s.kind==='alignment')return total+between(s.alignment_id,s.from,s.to);
    if(s.kind==='connected_alignments')return total+corridorSpans(s,world,lookup).reduce((sum,p)=>sum+span(p.alignmentId,p.begin,p.finish),0);
    throw Error('Unsupported physical segment');
   },0);}catch{/* Legacy links without geometry have no measurable length. */}
  }
  metersByRoute.set(routeId,length);return length;
 };
}
export function routeLength(route:Route,world:World):number|null{return indexRouteLengths(world)(route.id);}
export function RouteDirectory({world,enabled,selected,onChoose,onRename,onDirty}:{world:World;enabled:boolean;selected:string|null;onChoose:(id:string|null)=>void;onRename?:(id:string,name:string,expectedName:string)=>Promise<void>;onDirty?:(dirty:boolean)=>void}){
 const [edit,setEdit]=useState<{id:string;expectedName:string;name:string}|null>(null),[error,setError]=useState(''),[saving,setSaving]=useState(false),pending=useRef(false);
 const [query,setQuery]=useState('');
 const normalized=query.trim().normalize('NFKC').toLocaleLowerCase('ja');
 const editedId=edit?.id;
 const visible=useMemo(()=>normalized?world.routes.filter(route=>route.name.normalize('NFKC').toLocaleLowerCase('ja').includes(normalized)||route.id===editedId):world.routes,[world.routes,normalized,editedId]);
 const {route_paths,alignments,station_placements,alignment_connections}=world;
 const lengths=useMemo(()=>indexRouteLengths({route_paths,alignments,station_placements,alignment_connections}),[route_paths,alignments,station_placements,alignment_connections]);
 useEffect(()=>{onDirty?.(!!edit&&edit.name!==edit.expectedName);},[edit,onDirty]);
 useEffect(()=>()=>onDirty?.(false),[onDirty]);
 async function rename(){if(!edit||!onRename||!enabled||pending.current||!edit.name.trim())return;pending.current=true;setSaving(true);setError('');try{await onRename(edit.id,edit.name.trim(),edit.expectedName);setEdit(null);}catch(e){setError(String(e));}finally{pending.current=false;setSaving(false);}}
 return <div className="route-directory network-directory"><div className="directory-toolbar"><span className="directory-total">{world.routes.length}路線</span><button className="secondary" disabled={!enabled||!!edit} onClick={()=>onChoose(null)}>新規路線</button></div><label className="directory-search">路線を探す<input type="search" value={query} disabled={!!edit} placeholder="路線名" onChange={event=>setQuery(event.target.value)}/></label>
 <table aria-label="路線一覧"><thead><tr><th>名前</th><th>距離</th></tr></thead><tbody>{visible.map(route=>{const length=lengths(route.id);return <tr key={route.id}><th>
 {edit?.id===route.id?<form autoComplete="off" onSubmit={event=>{event.preventDefault();void rename();}}><input aria-label="変更後の路線名" autoComplete="off" maxLength={80} value={edit.name} disabled={saving} onChange={event=>setEdit({...edit,name:event.target.value})}/><button disabled={!enabled||saving||!edit.name.trim()||edit.name.trim()===edit.expectedName}>保存</button><button type="button" disabled={saving} onClick={()=>{setEdit(null);setError('');}}>取消</button>{error&&<p role="alert">{error}</p>}</form>:<div className="route-directory-name"><button className={`route-choice ${selected===route.id?'selected':''}`} aria-label={`編集: ${route.name}`} aria-pressed={selected===route.id} disabled={!enabled||!!edit} onClick={()=>onChoose(route.id)}>{route.name}</button>{onRename&&<button type="button" className="secondary" aria-label={`路線名を変更: ${route.name}`} disabled={!enabled||!!edit} onClick={()=>{setEdit({id:route.id,expectedName:route.name,name:route.name});setError('');}}>✎</button>}</div>}
 </th><td>{length===null?'—':`${(length/1000).toFixed(2)} km`}</td></tr>;})}</tbody></table>{!visible.length&&<p className="empty-state">{world.routes.length?"一致する路線がありません":"まだ路線はありません。新規路線から地図上の駅を選択してください。"}</p>}</div>;
}
