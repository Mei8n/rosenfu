import {assertNever} from './assertNever';
import {corridorStations,sameConnection} from "./connectedCorridor";
import type {EndpointConnection} from "./destinationJoin";
import type {Segment} from './RouteCandidatePreview';
export type StopReference={kind:'start'}|{kind:'on_leg';leg_key:string;station_id:string};
export type ServiceLeg={key:string;segment:Segment};
export type ServicePath={service_id:string;next_leg_key:string;legs:ServiceLeg[];stops:StopReference[]};
export type PathOccurrence={station_id:string;reference:StopReference};
export type PathPreview={legs:ServiceLeg[];occurrences:PathOccurrence[];requires_stop_selection:boolean};
export type PathWorld={alignment_connections?:EndpointConnection[];station_placements?:{station_id:string;alignment_id:string;position:number}[]};
export function referenceKey(ref:StopReference):string{
 switch(ref.kind){
  case 'start':return 'start';
  case 'on_leg':return `${ref.leg_key}:${ref.station_id}`;
  default:return assertNever(ref);
 }
}
export function resolveServicePath(start:string,legs:ServiceLeg[],world:PathWorld):PathOccurrence[]{
 const result:PathOccurrence[]=[{station_id:start,reference:{kind:'start'}}];let current=start;
 for(const leg of legs){const s=leg.segment;let ids:string[];
  switch(s.kind){
   case 'connected_alignments':{if(current!==s.from)throw Error('接続経路が連続していません');ids=corridorStations(s,world);break;}
   case 'alignment':{const ps=world.station_placements?.filter(p=>p.alignment_id===s.alignment_id)??[];const a=ps.find(p=>p.station_id===s.from),b=ps.find(p=>p.station_id===s.to);if(current!==s.from||!a||!b||a.position===b.position)throw Error('線形経路を解決できません');const span=ps.filter(p=>p.position>=Math.min(a.position,b.position)&&p.position<=Math.max(a.position,b.position)).sort((a,b)=>a.position-b.position);if(span.some((p,i)=>i>0&&p.position===span[i-1].position))throw Error('同位置の駅があります');if(a.position>b.position)span.reverse();ids=span.slice(1).map(p=>p.station_id);break;}
   default:return assertNever(s);
  }
  for(const id of ids){result.push({station_id:id,reference:{kind:'on_leg',leg_key:leg.key,station_id:id}});current=id;}
 }return result;
}

export function sameSegment(a:Segment,b:Segment):boolean{
 if(!a||!b||a.kind!==b.kind)return false;
 switch(a.kind){
  case 'connected_alignments':return b.kind==='connected_alignments'&&a.from===b.from&&a.to===b.to&&a.connections.length===b.connections.length&&a.connections.every((c,i)=>sameConnection(c,b.connections[i]));
  case 'alignment':return b.kind==='alignment'&&a.alignment_id===b.alignment_id&&a.from===b.from&&a.to===b.to;
  default:return assertNever(a);
 }
}
export function sameOccurrence(a:PathOccurrence,b:PathOccurrence):boolean{
 if(!a||!a.reference||a.station_id!==b.station_id||a.reference.kind!==b.reference.kind)return false;
 switch(a.reference.kind){
  case 'start':return true;
  case 'on_leg':return b.reference.kind==='on_leg'&&a.reference.leg_key===b.reference.leg_key&&a.reference.station_id===b.reference.station_id;
  default:return assertNever(a.reference);
 }
}
