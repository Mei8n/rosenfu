import {useEffect,useRef,useState} from 'react';
import {alignmentLength,type GeoPoint} from './alignment';
import {corridorSpans} from './connectedCorridor';
import type {EndpointConnection} from './destinationJoin';
import type {Segment} from './RouteCandidatePreview';
type Route={id:string;name:string;start:string};
type World={routes:Route[];alignments?:{id:string;points:GeoPoint[]}[];station_placements?:{station_id:string;alignment_id:string;position:number}[];alignment_connections?:EndpointConnection[];route_paths?:{route_id:string;segments:Segment[]}[]};
/** Physical spans only. Legacy links without geometry have no measurable length. */
export function routeLength(route:Route,world:World):number|null {
 const segments=world.route_paths?.find(p=>p.route_id===route.id)?.segments??[];
 if(!segments.length)return null;
 const placement=(id:string)=>world.station_placements?.find(p=>p.station_id===id);
 function span(id:string,begin:number,finish:number){
  const line=world.alignments?.find(a=>a.id===id);
  if(!line||!Number.isFinite(begin)||!Number.isFinite(finish)||begin<0||finish<0||begin>1||finish>1||begin===finish)throw Error('No measurable span');
  const length=alignmentLength(line.points)*Math.abs(finish-begin);
  if(!Number.isFinite(length)||length<=0)throw Error('No measurable length');
  return length;
 }
 function between(id:string,from:string,to:string){const a=placement(from),b=placement(to);if(!a||!b||a.alignment_id!==id||b.alignment_id!==id)throw Error('Missing placement');return span(id,a.position,b.position);}
 try{return segments.reduce((total,s)=>{
  if(s.kind==='alignment')return total+between(s.alignment_id,s.from,s.to);
  if(s.kind==='connected_alignments')return total+corridorSpans(s,world).reduce((sum,p)=>sum+span(p.alignmentId,p.begin,p.finish),0);
  throw Error('Unsupported physical segment');
 },0);}catch{return null;}
}
export function RouteDirectory({world,enabled,selected,onChoose,onRename,onDirty}:{world:World;enabled:boolean;selected:string|null;onChoose:(id:string|null)=>void;onRename?:(id:string,name:string,expectedName:string)=>Promise<void>;onDirty?:(dirty:boolean)=>void}){
 const [edit,setEdit]=useState<{id:string;expectedName:string;name:string}|null>(null),[error,setError]=useState(''),[saving,setSaving]=useState(false),pending=useRef(false);
 useEffect(()=>{onDirty?.(!!edit&&edit.name!==edit.expectedName);},[edit,onDirty]);
 useEffect(()=>()=>onDirty?.(false),[onDirty]);
 async function rename(){if(!edit||!onRename||!enabled||pending.current||!edit.name.trim())return;pending.current=true;setSaving(true);setError('');try{await onRename(edit.id,edit.name.trim(),edit.expectedName);setEdit(null);}catch(e){setError(String(e));}finally{pending.current=false;setSaving(false);}}
 return <div className="route-directory"><button className="secondary" disabled={!enabled||!!edit} onClick={()=>onChoose(null)}>新規路線</button>
 <table aria-label="路線一覧"><thead><tr><th>名前</th><th>距離</th></tr></thead><tbody>{world.routes.map(route=>{const length=routeLength(route,world);return <tr key={route.id}><th>
 {edit?.id===route.id?<form autoComplete="off" onSubmit={event=>{event.preventDefault();void rename();}}><input aria-label="変更後の路線名" autoComplete="off" maxLength={80} value={edit.name} disabled={saving} onChange={event=>setEdit({...edit,name:event.target.value})}/><button disabled={!enabled||saving||!edit.name.trim()||edit.name.trim()===edit.expectedName}>保存</button><button type="button" disabled={saving} onClick={()=>{setEdit(null);setError('');}}>取消</button>{error&&<p role="alert">{error}</p>}</form>:<><button className={`route-choice ${selected===route.id?'selected':''}`} aria-label={`編集: ${route.name}`} disabled={!enabled||!!edit} onClick={()=>onChoose(route.id)}>{route.name}</button>{onRename&&<button type="button" className="secondary" aria-label={`路線名を変更: ${route.name}`} disabled={!enabled||!!edit} onClick={()=>{setEdit({id:route.id,expectedName:route.name,name:route.name});setError('');}}>名前を変更</button>}</>}
 </th><td>{length===null?'—':`${(length/1000).toFixed(2)} km`}</td></tr>;})}</tbody></table></div>;
}
