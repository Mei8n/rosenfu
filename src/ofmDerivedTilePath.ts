import {findTileBoundaryContinuations,type TileRoadLine} from './ofmBoundaryContinuations';
type P=readonly [number,number];
export type TilePathPick={fragmentId:string;segmentIndex:number;fraction:number};
export type TilePathSpan={fragmentId:string;revision:string;rawCoordinates:P[];attributes:Pick<TileRoadLine,'class'|'subclass'|'access'|'brunnel'|'layer'|'level'|'oneway'>;from:number;to:number};
export type TilePathJoin={rule:'ofm_overlap_blend_v1';aId:string;bId:string;axis:0|1;fromAxis:number;toAxis:number;maxCorrectionMeters:number;points:P[];aRange:[number,number];bRange:[number,number]};
export type DerivedTilePath={points:P[];orderedSpans:TilePathSpan[];joins:TilePathJoin[];geometryDerived:true;segmentLimit:256};
export const coveredSourceSegments=(from:number,to:number)=>from===to?0:Math.ceil(Math.max(from,to))-Math.floor(Math.min(from,to));
const fail=(reason:string):never=>{throw Error(reason);};
const lat=(y:number)=>Math.atan(Math.sinh(Math.PI*(1-2*y/16384)))*180/Math.PI;
const at=(l:TileRoadLine,pos:number):P=>{const i=Math.min(Math.floor(pos),l.coordinates.length-2),t=pos-i,a=l.coordinates[i],b=l.coordinates[i+1];return [a[0]+(b[0]-a[0])*t,a[1]+(b[1]-a[1])*t];};
function location(l:TileRoadLine,axis:0|1,value:number):number{
 const hits:number[]=[];
 for(let i=0;i<l.coordinates.length-1;i++){const a=l.coordinates[i][axis],b=l.coordinates[i+1][axis];if(a===b){if(a===value)fail('boundary-parallel');continue;}const t=(value-a)/(b-a);if(t>=0&&t<=1&&!hits.some(p=>Math.abs(p-(i+t))<1e-8))hits.push(i+t);}
 if(hits.length!==1)return fail('ambiguous-boundary-crossing');return hits[0];
}
function slice(l:TileRoadLine,from:number,to:number):P[]{
 const direction=Math.sign(to-from);if(!direction)return [at(l,from)];
 const out:P[]=[at(l,from)];if(direction>0){for(let i=Math.floor(from)+1;i<to;i++)out.push([...l.coordinates[i]]);}else{for(let i=Math.ceil(from)-1;i>to;i--)out.push([...l.coordinates[i]]);}out.push(at(l,to));return out;
}
function join(a:TileRoadLine,b:TileRoadLine):TilePathJoin{
 const axis:0|1=a.x!==b.x?0:1;
 const middleLatitude=(lat(Math.max(a.y,b.y)+1)+lat(Math.min(a.y,b.y)))/2;
 const sx=111320*Math.cos(middleLatitude*Math.PI/180),sy=111320,scale=axis===0?sx:sy;
 const center=axis===0?Math.max(a.x,b.x)/16384*360-180:lat(Math.max(a.y,b.y));
 const direction=axis===0?Math.sign(b.x-a.x):Math.sign(a.y-b.y);
 const start=center-direction*10/scale,end=center+direction*10/scale;
 const ar:[number,number]=[location(a,axis,start),location(a,axis,end)],br:[number,number]=[location(b,axis,start),location(b,axis,end)];
 const values=new Set<number>([0,1]);for(let i=1;i<20;i++)values.add(i/20);
 for(const l of [a,b])for(const p of l.coordinates){const t=(p[axis]-start)/(end-start);if(t>0&&t<1)values.add(t);}
 if(values.size>257)fail('join-segment-limit');let maximum=0;const points:P[]=[];
 for(const t of [...values].sort((a,b)=>a-b)){
  const v=start+(end-start)*t,pa=at(a,location(a,axis,v)),pb=at(b,location(b,axis,v));
  const distance=Math.hypot((pa[0]-pb[0])*sx,(pa[1]-pb[1])*sy);if(distance>.8+1e-7)fail('correction-limit');maximum=Math.max(maximum,distance);
  const w=t*t*(3-2*t);points.push(t===0?pa:t===1?pb:[pa[0]+(pb[0]-pa[0])*w,pa[1]+(pb[1]-pa[1])*w]);
 }
 return {rule:'ofm_overlap_blend_v1',aId:a.id,bId:b.id,axis,fromAxis:start,toAxis:end,maxCorrectionMeters:maximum,points,aRange:ar,bRange:br};
}
/** Pure derived geometry; no source is rewritten or represented as exact copy.
 * Candidate evidence is always recomputed. No route through ambiguous branches. */
export function buildTileRoadPath(lines:readonly TileRoadLine[],start:TilePathPick,end:TilePathPick):DerivedTilePath{
 const checked=findTileBoundaryContinuations(lines);if(checked.truncated)fail('candidate-budget');
 const byId=new Map(lines.map(l=>[l.id,l]));if(byId.size!==lines.length)fail('duplicate-fragment');
 const validate=(p:TilePathPick)=>{const l=byId.get(p.fragmentId);if(!l||!Number.isInteger(p.segmentIndex)||p.segmentIndex<0||p.segmentIndex>=l.coordinates.length-1||!Number.isFinite(p.fraction)||p.fraction<0||p.fraction>1)fail('invalid-pick');return p.segmentIndex+p.fraction;};
 const startPos=validate(start),endPos=validate(end);
 if(lines.some(l=>l.revision!==lines[0]?.revision))fail('mixed-revision');
 if(start.fragmentId===end.fragmentId)fail('use-single-fragment-slice');
 const tiles=new Map<string,TileRoadLine[]>();for(const l of lines){const key=`${l.x}/${l.y}`;tiles.set(key,[...(tiles.get(key)??[]),l]);}if(tiles.size>64)fail('tile-budget');
 const groups=[...tiles.values()],pairs:typeof checked.pairs=[];
 for(let i=0;i<groups.length;i++)for(let j=i+1;j<groups.length;j++){
  const a=groups[i][0],b=groups[j][0];if(Math.abs(a.x-b.x)+Math.abs(a.y-b.y)!==1)continue;
  const result=findTileBoundaryContinuations([...groups[i],...groups[j]]);if(result.truncated)fail('candidate-budget');pairs.push(...result.pairs);
 }
 const adjacent=new Map<string,string[]>();for(const p of pairs){for(const [a,b]of [[p.aId,p.bId],[p.bId,p.aId]])adjacent.set(a,[...(adjacent.get(a)??[]),b]);}
 for(const list of adjacent.values())if(list.length>2)fail('ambiguous-branch');
 const parents=new Map<string,string|undefined>([[start.fragmentId,undefined]]),queue=[start.fragmentId];
 for(let i=0;i<queue.length;i++){
  const current=queue[i];for(const next of adjacent.get(current)??[]){if(next===parents.get(current))continue;if(parents.has(next))fail('cycle');parents.set(next,current);queue.push(next);if(queue.length>32)fail('fragment-limit');}
 }
 if(!parents.has(end.fragmentId))fail('disconnected');
 const chain:string[]=[];let current:string|undefined=end.fragmentId;while(current!==undefined){chain.unshift(current);current=parents.get(current);}
 const fragments=chain.map(id=>byId.get(id)!);if(fragments.some(l=>l.revision!==fragments[0].revision))fail('mixed-revision');
 const joins=fragments.slice(1).map((b,i)=>join(fragments[i],b));
 const points:P[]=[],spans:TilePathSpan[]=[];
 const append=(part:readonly P[])=>{for(const p of part){const last=points[points.length-1];if(!last||last[0]!==p[0]||last[1]!==p[1])points.push([...p]);if(points.length>257)fail('segment-limit');}};
 for(let i=0;i<fragments.length;i++){
  const l=fragments[i],before=joins[i-1],after=joins[i];const from=before?before.bRange[1]:startPos,to=after?after.aRange[0]:endPos;
  const direction=Math.sign(to-from);if(!direction)fail('pick-inside-blend');
  if(before&&Math.sign(before.bRange[1]-before.bRange[0])!==direction)fail('overlapping-joins-or-pick');
  if(after&&Math.sign(after.aRange[1]-after.aRange[0])!==direction)fail('overlapping-joins-or-pick');
  const spanFrom=before?before.bRange[0]:from,spanTo=after?after.aRange[1]:to;
  if(coveredSourceSegments(spanFrom,spanTo)>256)fail('source-segment-limit');
  spans.push({fragmentId:l.id,revision:l.revision,rawCoordinates:l.coordinates.map(p=>[...p]),attributes:{class:l.class,subclass:l.subclass,access:l.access,brunnel:l.brunnel,layer:l.layer,level:l.level,oneway:l.oneway},from:spanFrom,to:spanTo});
  append(slice(l,from,to));if(after)append(after.points);
 }
 if(spans.reduce((n,s)=>n+coveredSourceSegments(s.from,s.to),0)>256)fail('source-segment-limit');
 return {points,orderedSpans:spans,joins,geometryDerived:true,segmentLimit:256};
}
