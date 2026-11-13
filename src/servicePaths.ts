import type {Segment} from './RouteCandidatePreview';
export type StopReference={kind:'start'}|{kind:'on_leg';leg_key:string;station_id:string};
export type ServiceLeg={key:string;segment:Segment};
export type ServicePath={service_id:string;next_leg_key:string;legs:ServiceLeg[];stops:StopReference[]};
export type PathOccurrence={station_id:string;reference:StopReference};
export type PathPreview={legs:ServiceLeg[];occurrences:PathOccurrence[];requires_stop_selection:boolean};
export type PathWorld={tracks:{id:string;from:string;to:string}[];station_placements?:{station_id:string;alignment_id:string;position:number}[]};
export function referenceKey(ref:StopReference){return ref.kind==='start'?'start':`${ref.leg_key}:${ref.station_id}`;}
export function resolveServicePath(start:string,legs:ServiceLeg[],world:PathWorld):PathOccurrence[]{
 const result:PathOccurrence[]=[{station_id:start,reference:{kind:'start'}}];let current=start;
 for(const leg of legs){const s=leg.segment;let ids:string[];
  if(s.kind==='track'){const t=world.tracks.find(t=>t.id===s.track_id);if(!t||(t.from!==current&&t.to!==current))throw Error('経路を解決できません');ids=[t.from===current?t.to:t.from];}
  else{const ps=world.station_placements?.filter(p=>p.alignment_id===s.alignment_id)??[];const a=ps.find(p=>p.station_id===s.from),b=ps.find(p=>p.station_id===s.to);if(current!==s.from||!a||!b||a.position===b.position)throw Error('線形経路を解決できません');const span=ps.filter(p=>p.position>=Math.min(a.position,b.position)&&p.position<=Math.max(a.position,b.position)).sort((a,b)=>a.position-b.position);if(span.some((p,i)=>i>0&&p.position===span[i-1].position))throw Error('同位置の駅があります');if(a.position>b.position)span.reverse();ids=span.slice(1).map(p=>p.station_id);}
  for(const id of ids){result.push({station_id:id,reference:{kind:'on_leg',leg_key:leg.key,station_id:id}});current=id;}
 }return result;
}

export function sameSegment(a:Segment,b:Segment):boolean{
 if(!a||!b||a.kind!==b.kind)return false;
 return a.kind==='track'&&b.kind==='track'?a.track_id===b.track_id:a.kind==='alignment'&&b.kind==='alignment'&&a.alignment_id===b.alignment_id&&a.from===b.from&&a.to===b.to;
}
export function sameOccurrence(a:PathOccurrence,b:PathOccurrence):boolean{
 if(!a||!a.reference||a.station_id!==b.station_id||a.reference.kind!==b.reference.kind)return false;
 return a.reference.kind==='start'||(a.reference.kind==='on_leg'&&b.reference.kind==='on_leg'&&a.reference.leg_key===b.reference.leg_key&&a.reference.station_id===b.reference.station_id);
}
