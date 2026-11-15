import {alignmentLength,type GeoPoint} from './alignment';
import {corridorSpans} from './connectedCorridor';
import type {EndpointConnection} from './destinationJoin';
import type {Segment} from './RouteCandidatePreview';
type Route={id:string;name:string;start:string;tracks:string[]};
type World={routes:Route[];tracks:{id:string;from:string;to:string}[];alignments?:{id:string;points:GeoPoint[]}[];station_placements?:{station_id:string;alignment_id:string;position:number}[];alignment_connections?:EndpointConnection[];route_paths?:{route_id:string;segments:Segment[]}[]};
/** Physical spans only. Legacy links without geometry have no measurable length. */
export function routeLength(route:Route,world:World):number|null {
 const segments=world.route_paths?.find(p=>p.route_id===route.id)?.segments??route.tracks.map(track_id=>({kind:'track' as const,track_id}));
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
  // Station placement is not Track geometry: a legacy link never records
  // which physical alignment (if any) it follows. Do not infer that relation.
  throw Error('Legacy link has no physical geometry');
 },0);}catch{return null;}
}
export function RouteDirectory({world,enabled,selected,onChoose}:{world:World;enabled:boolean;selected:string|null;onChoose:(id:string|null)=>void}){
 return <div className="route-directory"><button className="secondary" disabled={!enabled} onClick={()=>onChoose(null)}>新規路線</button>
 <table aria-label="路線一覧"><thead><tr><th>名前</th><th>距離</th><th>乗客</th><th>収支</th><th>車両数</th></tr></thead><tbody>{world.routes.map(route=>{const length=routeLength(route,world);return <tr key={route.id}><th><button className={`route-choice ${selected===route.id?'selected':''}`} aria-label={`編集: ${route.name}`} disabled={!enabled} onClick={()=>onChoose(route.id)}>{route.name}</button></th><td>{length===null?'—':`${(length/1000).toFixed(2)} km`}</td>{['乗客','収支','車両数'].map(label=><td key={label} title="未実装" aria-label={`${label}: 未実装`}>—</td>)}</tr>;})}</tbody></table></div>;
}
