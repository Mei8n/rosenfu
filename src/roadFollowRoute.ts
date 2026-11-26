import {eligibleRoadGraph} from './roadFollowing';
import type {ReferenceGraphBundle} from './referenceGraphBundle';
import type {GraphPoint} from './referenceGraph';
import type {ReferenceGraphPick} from './referenceGraphPath';

/** Geometric drawing assistance only. Never establishes a physical rail junction,
 * legal tram access, or source provenance. Generated points still need the normal
 * construction validator. This is independent of the exact-source-copy pipeline. */
export function createRoadFollowRouter(bundle:ReferenceGraphBundle,exactSourceNodes=false){
 const graph=eligibleRoadGraph(bundle),nodes=new Map(graph.nodes.map(n=>[n.osmId,n])),edges=new Map(graph.edges.map(e=>[e.id,e]));
 const distance=(a:GraphPoint,b:GraphPoint)=>{const r=Math.PI/180,dlat=(b.latitude-a.latitude)*r,dlon=(b.longitude-a.longitude)*r;const h=Math.sin(dlat/2)**2+Math.cos(a.latitude*r)*Math.cos(b.latitude*r)*Math.sin(dlon/2)**2;return 12742000*Math.asin(Math.sqrt(Math.min(1,h)));};
 // Exact vertices may connect only within the same encoded level group. Missing
 // metadata stays in its own group; crossings without vertices never connect.
 const key=(id:string,edge:typeof graph.edges[number])=>{const p=nodes.get(id)!;return `vertex:${edge.layer??"unspecified"}:${edge.level??"unspecified"}:${exactSourceNodes?id:`${p.longitude}:${p.latitude}`}`;};
 type Arc={to:string;length:number};
 const baseLocations=new Map<string,GraphPoint>(),baseAdjacent=new Map<string,Arc[]>(),ends=new Map<string,{a:string;b:string}>();
 for(const edge of graph.edges){const a=key(edge.from,edge),b=key(edge.to,edge);baseLocations.set(a,nodes.get(edge.from)!);baseLocations.set(b,nodes.get(edge.to)!);ends.set(edge.id,{a,b});const length=distance(nodes.get(edge.from)!,nodes.get(edge.to)!);baseAdjacent.set(a,[...(baseAdjacent.get(a)??[]),{to:b,length}]);baseAdjacent.set(b,[...(baseAdjacent.get(b)??[]),{to:a,length}]);}
 return function route(start:ReferenceGraphPick,end:ReferenceGraphPick):{points:GraphPoint[];lengthM:number}{
 const pointAt=(pick:ReferenceGraphPick)=>{
  const edge=edges.get(pick.edgeId);
  if(!edge||pick.datasetId!==graph.dataset.id||pick.revision!==graph.dataset.revision||!Number.isFinite(pick.fraction)||pick.fraction<0||pick.fraction>1)throw Error('道路の選択位置を確認してください');
  const a=nodes.get(edge.from)!,b=nodes.get(edge.to)!;
  return {longitude:a.longitude+(b.longitude-a.longitude)*pick.fraction,latitude:a.latitude+(b.latitude-a.latitude)*pick.fraction};
 };
 const startPoint=pointAt(start),endPoint=pointAt(end),virtual=new Map([['@start',startPoint],['@end',endPoint]]);
 const location=(id:string)=>virtual.get(id)??baseLocations.get(id)!;
 // Overlay only the two picks. The source graph and its adjacency are prepared
 // once per data revision, not revalidated/reallocated on every pointer move.
 const extra=new Map<string,Arc[]>();
 const add=(a:string,b:string)=>{const length=distance(location(a),location(b));extra.set(a,[...(extra.get(a)??[]),{to:b,length}]);extra.set(b,[...(extra.get(b)??[]),{to:a,length}]);};
 for(const [id,pick] of [['@start',start],['@end',end]] as const){const e=ends.get(pick.edgeId)!;add(id,e.a);add(id,e.b);}
 if(start.edgeId===end.edgeId)add('@start','@end');
 // Binary-heap Dijkstra: distance, rather than fragment count, chooses branches.
 const heap:{id:string;cost:number}[]=[];
 const push=(entry:typeof heap[number])=>{heap.push(entry);let i=heap.length-1;while(i){const parent=(i-1)>>1;if(heap[parent].cost<=entry.cost)break;heap[i]=heap[parent];i=parent;}heap[i]=entry;};
 const pop=()=>{const first=heap[0],last=heap.pop()!;if(heap.length){let i=0;while(i*2+1<heap.length){let child=i*2+1;if(child+1<heap.length&&heap[child+1].cost<heap[child].cost)child++;if(heap[child].cost>=last.cost)break;heap[i]=heap[child];i=child;}heap[i]=last;}return first;};
 const costs=new Map<string,number>([['@start',0]]),parents=new Map<string,string>();push({id:'@start',cost:0});
 while(heap.length){const current=pop();if(current.cost!==costs.get(current.id))continue;if(current.id==='@end')break;for(const next of [...(baseAdjacent.get(current.id)??[]),...(extra.get(current.id)??[])]){const cost=current.cost+next.length;if(cost<(costs.get(next.to)??Infinity)){costs.set(next.to,cost);parents.set(next.to,current.id);push({id:next.to,cost});}}}
 const lengthM=costs.get('@end');if(lengthM===undefined)throw Error('この地点までつながる道路が見つかりません');
 const path:string[]=['@end'];while(path.at(-1)!=='@start'){const parent=parents.get(path.at(-1)!);if(!parent)throw Error('道路経路を再選択してください');path.push(parent);}path.reverse();
 const points=path.map(location).filter((p,i,all)=>i===0||p.longitude!==all[i-1].longitude||p.latitude!==all[i-1].latitude).map(p=>({latitude:p.latitude,longitude:p.longitude}));
 return {points,lengthM};
}
}
export function roadFollowRoute(bundle:ReferenceGraphBundle,start:ReferenceGraphPick,end:ReferenceGraphPick){return createRoadFollowRouter(bundle)(start,end);}
