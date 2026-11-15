import type {GraphBounds, GraphPoint} from './referenceGraph';
import {createReferenceSpatialIndex} from './referenceSpatialIndex';
import {distanceM} from './alignment';

// Independently implemented from the published TWRG v1 binary format description.
// IDs below identify this versioned file's junctions/chains, NOT OSM ways.
export type RailChain={id:number;from:number;to:number;points:GraphPoint[];positions:number[];length:number;bounds:GraphBounds};
export type RailGraph={chains:RailChain[];junctionCount:number};
export type RailPick={chain:number;position:number;point:GraphPoint};
export type RailSpan={region:string;chain_id:number;from_fraction:number;to_fraction:number};
export type RailRoute={points:GraphPoint[];spans:RailSpan[]};
export const RAIL_MAX_CHAINS=256, RAIL_MAX_POINTS=4096;
export function decodeRailGraph(raw:ArrayBuffer):RailGraph {
 const view=new DataView(raw);let cursor=0;
 const take=(bytes:number)=>{if(cursor+bytes>view.byteLength)throw Error('線路データが途中で切れています');const at=cursor;cursor+=bytes;return at;};
 const uint=()=>view.getUint32(take(4),true),int=()=>view.getInt32(take(4),true),byte=()=>view.getUint8(take(1));
 if(uint()!==0x47525754||uint()!==1)throw Error('非対応の線路データ形式');
 const junctionCount=uint(),count=uint(),vertices=uint(),shared=uint();
 if(junctionCount>500000||count>600000||vertices>3000000||shared>junctionCount||raw.byteLength>32*1024*1024)throw Error('線路データ上限を超えています');
 const read=(n:number,fn:()=>number)=>Array.from({length:n},fn);
 const jx=read(junctionCount,int),jy=read(junctionCount,int),from=read(count,uint),to=read(count,uint);
 read(count,byte);read(count,byte);read(count,byte); // kind/rank/flags never become game height
 const offsets=read(count+1,uint);
 if(offsets[0]!==0||offsets[count]!==vertices)throw Error('不正な線路頂点範囲');
 const delta=()=>{let result=0,factor=1;for(let n=0;n<5;n++){const b=byte();result+=(b&127)*factor;if(!(b&128))return result%2?-(result+1)/2:result/2;factor*=128;}throw Error('不正な線路座標');};
 const point=(x:number,y:number):GraphPoint=>{if(!Number.isSafeInteger(x)||!Number.isSafeInteger(y)||Math.abs(x)>180e6||Math.abs(y)>85.051128e6)throw Error('不正な線路座標');return {longitude:x/1e6,latitude:y/1e6};};
 const chains:RailChain[]=[];
 for(let id=0;id<count;id++){
  const size=offsets[id+1]-offsets[id];
  if(size<2||size>vertices||from[id]>=junctionCount||to[id]>=junctionCount)throw Error('不正な線路接続');
  let x=0,y=0,length=0;const points:GraphPoint[]=[],positions:number[]=[];
  const bounds={west:180,east:-180,south:90,north:-90};
  for(let k=0;k<size;k++){x+=delta();y+=delta();const p=point(x,y);if(k)length+=distanceM(points[k-1],p);points.push(p);positions.push(length);bounds.west=Math.min(bounds.west,p.longitude);bounds.east=Math.max(bounds.east,p.longitude);bounds.south=Math.min(bounds.south,p.latitude);bounds.north=Math.max(bounds.north,p.latitude);}
  const a=points[0],b=points[size-1];
  if(a.longitude!==jx[from[id]]/1e6||a.latitude!==jy[from[id]]/1e6||b.longitude!==jx[to[id]]/1e6||b.latitude!==jy[to[id]]/1e6)throw Error('線路端点と接続節点が一致しません');
  chains.push({id,from:from[id],to:to[id],points,positions:positions.map(p=>length?p/length:0),length,bounds});
 }
 // The Japan-only provider has no adjacent-region loading. Validate the shared
 // table but never invent cross-region connections from coordinate proximity.
 const sharedNodes=read(shared,uint);const seen=new Set<number>();
 for(const node of sharedNodes){const osm=view.getFloat64(take(8),true);if(node>=junctionCount||seen.has(node)||!Number.isSafeInteger(osm)||osm<=0)throw Error('不正な地域境界接続');seen.add(node);}
 if(cursor!==view.byteLength)throw Error('線路データ末尾が不正です');
 return {chains,junctionCount};
}
function at(chain:RailChain,t:number):GraphPoint {
 if(t===0)return {...chain.points[0]};if(t===1)return {...chain.points.at(-1)!};
 for(let i=1;i<chain.points.length;i++)if(chain.positions[i]>=t){const f=(t-chain.positions[i-1])/(chain.positions[i]-chain.positions[i-1]);const a=chain.points[i-1],b=chain.points[i];return {longitude:a.longitude+f*(b.longitude-a.longitude),latitude:a.latitude+f*(b.latitude-a.latitude)};}
 throw Error('不正な参照位置');
}
export function createRailNetwork(graph:RailGraph){
 const spatial=createReferenceSpatialIndex(graph.chains.map(c=>c.bounds));
 const incident:number[][]=Array.from({length:graph.junctionCount},()=>[]);
 for(const chain of graph.chains){if(!chain.length)continue;incident[chain.from].push(chain.id);incident[chain.to].push(chain.id);}
 function visible(bounds:GraphBounds){const ids=spatial.query(bounds).filter(id=>graph.chains[id].length>0);if(ids.length>2000)return {chains:[],truncated:true};return {chains:ids.map(id=>graph.chains[id]),truncated:false};}
 function nearest(point:GraphPoint,radius:number,visibleIds:number[]):RailPick|null {
  if(!Number.isFinite(radius)||radius<=0||radius>1000)return null;
  let best=radius,result:RailPick|null=null;const scaleX=111195*Math.cos(point.latitude*Math.PI/180),scaleY=111195;
  for(const id of visibleIds){const c=graph.chains[id];if(!c)continue;for(let i=1;i<c.points.length;i++){const a=c.points[i-1],b=c.points[i],dx=(b.longitude-a.longitude)*scaleX,dy=(b.latitude-a.latitude)*scaleY,ax=(point.longitude-a.longitude)*scaleX,ay=(point.latitude-a.latitude)*scaleY,l=dx*dx+dy*dy;if(!l)continue;const f=Math.max(0,Math.min(1,(ax*dx+ay*dy)/l)),d=Math.hypot(ax-f*dx,ay-f*dy);if(d<best){best=d;const position=c.positions[i-1]+f*(c.positions[i]-c.positions[i-1]);result={chain:id,position,point:at(c,position)};}}}
  return result;
 }
 function route(start:RailPick,end:RailPick):RailRoute {
  for(const p of [start,end])if(!graph.chains[p.chain]?.length||!Number.isFinite(p.position)||p.position<0||p.position>1)throw Error('不正な参照点');
  const first=graph.chains[start.chain],last=graph.chains[end.chain];
  type Step={node:number;cost:number};const heap:Step[]=[];
  const push=(s:Step)=>{heap.push(s);let i=heap.length-1;while(i){const p=(i-1)>>1;if(heap[p].cost<=s.cost)break;heap[i]=heap[p];i=p;}heap[i]=s;};
  const pop=()=>{const result=heap[0],tail=heap.pop()!;if(heap.length){let i=0;while(i*2+1<heap.length){let child=i*2+1;if(child+1<heap.length&&heap[child+1].cost<heap[child].cost)child++;if(heap[child].cost>=tail.cost)break;heap[i]=heap[child];i=child;}heap[i]=tail;}return result;};
  const costs=new Float64Array(graph.junctionCount).fill(Infinity),previous=new Map<number,{node:number;chain:number;from:number;to:number}>();
  for(const [node,fraction] of [[first.from,0],[first.to,1]]){const cost=Math.abs(start.position-fraction)*first.length;if(cost<costs[node]){costs[node]=cost;previous.set(node,{node:-1,chain:first.id,from:start.position,to:fraction});push({node,cost});}}
  let best=first.id===last.id?Math.abs(start.position-end.position)*first.length:Infinity,finish=-1,finishFraction=0,visited=0;
  while(heap.length){const s=pop();if(s.cost!==costs[s.node])continue;if(s.cost>=best)break;if(++visited>200000)throw Error('探索上限です。区間を短くしてください');
   for(const [node,fraction] of [[last.from,0],[last.to,1]])if(s.node===node){const total=s.cost+Math.abs(end.position-fraction)*last.length;if(total<best){best=total;finish=node;finishFraction=fraction;}}
   for(const id of incident[s.node]){const c=graph.chains[id],forward=c.from===s.node,node=forward?c.to:c.from,cost=s.cost+c.length;if(cost<costs[node]){costs[node]=cost;previous.set(node,{node:s.node,chain:id,from:forward?0:1,to:forward?1:0});push({node,cost});}}
  }
  if(!Number.isFinite(best))throw Error('取得済み線路網では接続する経路がありません（データ欠損の可能性があります）');
  const spans:RailSpan[]=[];
  if(finish===-1)spans.push({region:'jp',chain_id:first.id,from_fraction:start.position,to_fraction:end.position});
  else {let node=finish;while(node!==-1){const step=previous.get(node)!;spans.unshift({region:'jp',chain_id:step.chain,from_fraction:step.from,to_fraction:step.to});node=step.node;if(spans.length>RAIL_MAX_CHAINS)throw Error('最大256区間です。区間を短くしてください');}spans.push({region:'jp',chain_id:last.id,from_fraction:finishFraction,to_fraction:end.position});}
  const nonempty=spans.filter(s=>s.from_fraction!==s.to_fraction);if(nonempty.length>RAIL_MAX_CHAINS)throw Error('最大256区間です');
  const points:GraphPoint[]=[];
  for(const s of nonempty){const c=graph.chains[s.chain_id],lo=Math.min(s.from_fraction,s.to_fraction),hi=Math.max(s.from_fraction,s.to_fraction),middle=c.points.filter((_,i)=>c.positions[i]>lo&&c.positions[i]<hi),part=[at(c,s.from_fraction),...(s.from_fraction<s.to_fraction?middle:middle.reverse()),at(c,s.to_fraction)];for(const p of part){const prev=points.at(-1);if(!prev||prev.longitude!==p.longitude||prev.latitude!==p.latitude)points.push({...p});}}
  if(points.length<2||best<20)throw Error('建設長が20m未満です');if(points.length>RAIL_MAX_POINTS)throw Error('形状点が多すぎます。区間を短くしてください');
  return {points,spans:nonempty};
 }
 return {visible,nearest,route};
}
