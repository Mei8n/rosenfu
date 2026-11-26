/** Conservative geometric continuity candidates, NOT topology or permission.
 * Never mutates/joins geometry. A caller must preserve all source provenance. */
export type TileRoadLine={id:string;revision:string;z:14;x:number;y:number;coordinates:readonly (readonly [number,number])[];class:string;access?:string;subclass?:string;brunnel?:string;layer?:number;level?:string;oneway?:number|string};
export type BoundaryContinuation={aId:string;bId:string;overlapMeters:number;maxSeparationMeters:number};
type P=[number,number];
const MAX_LINES=2000,MAX_POINTS=100000,MAX_PAIRS=10000,TOLERANCE=.8,BAND=10;
const classes=new Set(['primary','secondary','tertiary','minor','service']);
const lat=(y:number)=>Math.atan(Math.sinh(Math.PI*(1-2*y/16384)))*180/Math.PI;
const dist=(a:P,b:P)=>Math.hypot(a[0]-b[0],a[1]-b[1]);
const length=(p:P[])=>p.slice(1).reduce((s,q,i)=>s+dist(q,p[i]),0);
function eligible(l:TileRoadLine){return (l.oneway===undefined||(typeof l.oneway==='number'&&[0,1,-1].includes(l.oneway)))&&classes.has(l.class)&&!l.brunnel&&(l.layer===undefined||l.layer===0)&&(l.level===undefined||/^[+-]?0(?:\.0+)?$/.test(l.level));}
function same(a:TileRoadLine,b:TileRoadLine){return a.access===b.access&&a.class===b.class&&a.subclass===b.subclass&&a.brunnel===b.brunnel&&a.layer===b.layer&&a.level===b.level&&a.oneway===b.oneway;}
/** One monotonic boundary passage only; loops/re-entry and corner cases refused. */
function strip(points:P[],axis:number,seam:number):P[]|undefined{
 const out:P[]=[];let direction=0;
 for(let i=1;i<points.length;i++){
  const a=points[i-1],b=points[i],d=b[axis]-a[axis];
  if(!d){if(Math.abs(a[axis]-seam)<=BAND)return;continue;}
  let t0=(seam-BAND-a[axis])/d,t1=(seam+BAND-a[axis])/d;
  if(t0>t1)[t0,t1]=[t1,t0];t0=Math.max(0,t0);t1=Math.min(1,t1);if(t1<=t0)continue;
  if(direction&&direction!==Math.sign(d))return;direction=Math.sign(d);
  const p:P=[a[0]+t0*(b[0]-a[0]),a[1]+t0*(b[1]-a[1])],q:P=[a[0]+t1*(b[0]-a[0]),a[1]+t1*(b[1]-a[1])];
  if(out.length&&dist(out[out.length-1],p)>1e-5)return;
  if(!out.length)out.push(p);out.push(q);
 }
 if(out.length<2||Math.abs(Math.abs(out[0][axis]-seam)-BAND)>1e-5||Math.abs(Math.abs(out[out.length-1][axis]-seam)-BAND)>1e-5||out[0][axis]===out[out.length-1][axis])return;
 if(out[0][axis]>out[out.length-1][axis])out.reverse();return out;
}
function pointSegment(p:P,a:P,b:P){const dx=b[0]-a[0],dy=b[1]-a[1],den=dx*dx+dy*dy,t=den?Math.max(0,Math.min(1,((p[0]-a[0])*dx+(p[1]-a[1])*dy)/den)):0;return Math.hypot(p[0]-a[0]-t*dx,p[1]-a[1]-t*dy);}
/** Continuous upper bound: distance to one convex target segment is convex
 * along a source segment, hence its endpoint maximum bounds the entire segment.
 * Min across those valid upper bounds remains an upper bound to the polyline. */
export function continuousSeparation(a:P[],b:P[],budget={remaining:1000000}):number{
 const bound=(p:P,q:P,depth:number):number=>{
  let upper=Infinity;
  for(let j=1;j<b.length;j++){if(--budget.remaining<0)return Infinity;upper=Math.min(upper,Math.max(pointSegment(p,b[j-1],b[j]),pointSegment(q,b[j-1],b[j])));}
  if(upper<=TOLERANCE||depth===8||budget.remaining<0)return upper;
  const middle:P=[(p[0]+q[0])/2,(p[1]+q[1])/2];
  return Math.max(bound(p,middle,depth+1),bound(middle,q,depth+1));
 };
 let worst=0;for(let i=1;i<a.length;i++){worst=Math.max(worst,bound(a[i-1],a[i],0));if(worst>TOLERANCE||budget.remaining<0)return worst;}return worst;
}
export function findTileBoundaryContinuations(lines:readonly TileRoadLine[]):{pairs:BoundaryContinuation[];truncated:boolean}{
 if(lines.length>MAX_LINES||lines.reduce((n,l)=>n+l.coordinates.length,0)>MAX_POINTS)return {pairs:[],truncated:true};
 const ids=new Set<string>();
 for(const l of lines){if(ids.has(l.id)||!l.id||!l.revision||l.z!==14||![l.x,l.y].every(v=>Number.isInteger(v)&&v>=0&&v<16384)||l.coordinates.length<2||l.coordinates.length>512||l.coordinates.some(p=>!p.every(Number.isFinite)||Math.abs(p[0])>180||Math.abs(p[1])>85))return {pairs:[],truncated:false};ids.add(l.id);}
 const candidates:BoundaryContinuation[]=[];let checked=0;const distanceBudget={remaining:1000000};
 const cache=new Map<string,{points:P[];clipped:P[]|undefined}>();
 for(let i=0;i<lines.length;i++)for(let j=i+1;j<lines.length;j++){
  const a=lines[i],b=lines[j];if(a.revision!==b.revision||Math.abs(a.x-b.x)+Math.abs(a.y-b.y)!==1||!eligible(a)||!eligible(b)||!same(a,b))continue;
  const vertical=a.x!==b.x;const axis=vertical?0:1;
  const south=lat(Math.max(a.y,b.y)+1),north=lat(Math.min(a.y,b.y));const sy=111320,sx=sy*Math.cos((south+north)/2*Math.PI/180);
  const seam=vertical?(Math.max(a.x,b.x)/16384*360-180)*sx:lat(Math.max(a.y,b.y))*sy;
  const project=(l:TileRoadLine)=>l.coordinates.map(([x,y])=>[x*sx,y*sy] as P);
  const geometry=(l:TileRoadLine)=>{const key=`${l.id}/${axis}/${seam}`;let value=cache.get(key);if(!value){const points=project(l);value={points,clipped:strip(points,axis,seam)};cache.set(key,value);}return value;};
  const ga=geometry(a),gb=geometry(b),pa=ga.points,pb=gb.points,aa=ga.clipped,bb=gb.clipped;if(!aa||!bb||length(aa)>100||length(bb)>100)continue;
  if(a.oneway!==undefined&&a.oneway!==0&&Math.sign(pa[pa.length-1][axis]-pa[0][axis])!==Math.sign(pb[pb.length-1][axis]-pb[0][axis]))continue;
  const low=vertical?south*sy:(Math.max(a.x,b.x)/16384*360-180)*sx;
  const high=vertical?north*sy:((Math.min(a.x,b.x)+1)/16384*360-180)*sx;
  if([...aa,...bb].some(p=>p[1-axis]<low+BAND||p[1-axis]>high-BAND))continue;
  if(++checked>MAX_PAIRS)return {pairs:[],truncated:true};
  const tangent=(p:P[])=>{const x=p[p.length-1][0]-p[0][0],y=p[p.length-1][1]-p[0][1],n=Math.hypot(x,y);return [x/n,y/n];};
  const ta=tangent(aa),tb=tangent(bb);if(ta[0]*tb[0]+ta[1]*tb[1]<.995)continue;
  const error=Math.max(continuousSeparation(aa,bb,distanceBudget),continuousSeparation(bb,aa,distanceBudget));if(distanceBudget.remaining<0)return {pairs:[],truncated:true};if(error>TOLERANCE)continue;
  candidates.push({aId:a.id,bId:b.id,overlapMeters:Math.min(length(aa),length(bb)),maxSeparationMeters:error});
 }
 const counts=new Map<string,number>();for(const p of candidates)for(const id of [p.aId,p.bId])counts.set(id,(counts.get(id)??0)+1);
 return {pairs:candidates.filter(p=>counts.get(p.aId)===1&&counts.get(p.bId)===1),truncated:false};
}
