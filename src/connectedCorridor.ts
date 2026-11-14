import type {EndpointConnection} from "./destinationJoin";
import type {Segment} from "./RouteCandidatePreview";
type Placement={station_id:string;alignment_id:string;position:number};
type CorridorWorld={station_placements?:Placement[];alignment_connections?:EndpointConnection[]};
const sameEnd=(a:EndpointConnection["a"],b:EndpointConnection["a"])=>a.alignment_id===b.alignment_id&&a.endpoint===b.endpoint;
export function sameConnection(a:EndpointConnection,b:EndpointConnection){return sameEnd(a.a,b.a)&&sameEnd(a.b,b.b);}
export function corridorSpans(segment:Extract<Segment,{kind:"connected_alignments"}>,world:CorridorWorld){
 const from=world.station_placements?.find(p=>p.station_id===segment.from),to=world.station_placements?.find(p=>p.station_id===segment.to);
 if(!from||!to||segment.from===segment.to||!segment.connections.length||segment.connections.length>256)throw Error("接続経路が無効です");
 const spans:{alignmentId:string;begin:number;finish:number}[]=[],seen=new Set<string>();
 let id=from.alignment_id,begin=from.position;
 for(const link of segment.connections){
  if(link.a.alignment_id!==id||seen.has(id)||!world.alignment_connections?.some(c=>sameConnection(c,link)||sameConnection({a:c.b,b:c.a},link)))throw Error("線路の接続を確認できません");
  seen.add(id);const finish=link.a.endpoint==="start"?0:1;
  if(begin===finish)throw Error("接続区間がありません");
  spans.push({alignmentId:id,begin,finish});id=link.b.alignment_id;begin=link.b.endpoint==="start"?0:1;
 }
 const ringReturn=id===from.alignment_id&&((segment.connections[0].a.endpoint==="start"&&begin===1&&to.position>from.position)||(segment.connections[0].a.endpoint==="end"&&begin===0&&to.position<from.position));
 if(id!==to.alignment_id||(seen.has(id)&&!ringReturn)||begin===to.position)throw Error("接続先が一致しません");
 spans.push({alignmentId:id,begin,finish:to.position});return spans;
}
export function corridorStations(segment:Extract<Segment,{kind:"connected_alignments"}>,world:CorridorWorld){
 return corridorSpans(segment,world).flatMap(span=>{
  const ps=(world.station_placements??[]).filter(p=>p.alignment_id===span.alignmentId&&p.position>=Math.min(span.begin,span.finish)&&p.position<=Math.max(span.begin,span.finish)).sort((a,b)=>a.position-b.position);
  if(ps.some((p,i)=>i>0&&p.position===ps[i-1].position))throw Error("同位置の駅があります");
  if(span.begin>span.finish)ps.reverse();return ps.filter(p=>p.station_id!==segment.from).map(p=>p.station_id);
 });
}
