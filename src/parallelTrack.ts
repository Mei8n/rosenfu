import type {GeoPoint} from './alignment';
import {sampledGeometry,type EditedGeometry} from './editedGeometry';
export const PARALLEL_RANGE_M=30;
const R=6371000,rad=Math.PI/180;
function plane(origin:GeoPoint){const k=R*rad,c=Math.cos(origin.latitude*rad);return {xy:(p:GeoPoint)=>({x:(p.longitude-origin.longitude)*k*c,y:(p.latitude-origin.latitude)*k}),geo:(p:{x:number;y:number}):GeoPoint=>({longitude:origin.longitude+p.x/(k*c),latitude:origin.latitude+p.y/k})};}
/** Project onto a sampled alignment, retaining arc length and side. */
export function parallelProjection(line:GeoPoint[],point:GeoPoint){
 if(line.length<2)return null;const p=plane(line[0]),q=p.xy(point);let best:null|{distance:number;along:number;offset:number;point:GeoPoint}=null,total=0;
 for(let i=1;i<line.length;i++){const a=p.xy(line[i-1]),b=p.xy(line[i]),dx=b.x-a.x,dy=b.y-a.y,length=Math.hypot(dx,dy);if(length<.001)continue;const t=Math.max(0,Math.min(1,((q.x-a.x)*dx+(q.y-a.y)*dy)/(length*length))),v={x:a.x+t*dx,y:a.y+t*dy},distance=Math.hypot(q.x-v.x,q.y-v.y);if(!best||distance<best.distance)best={distance,along:total+t*length,offset:(dx*(q.y-v.y)-dy*(q.x-v.x))/length,point:p.geo(v)};total+=length;}return best;
}
export function offsetTrack(line:GeoPoint[],offset:number):GeoPoint[]|null{
 if(line.length<2||Math.abs(offset)<3||Math.abs(offset)>PARALLEL_RANGE_M)return null;const p=plane(line[0]),xy=line.map(p.xy),result:GeoPoint[]=[];
 const normal=(a:{x:number;y:number},b:{x:number;y:number})=>{const l=Math.hypot(b.x-a.x,b.y-a.y);return l<.001?null:{x:-(b.y-a.y)/l,y:(b.x-a.x)/l};};
 for(let i=0;i<xy.length;i++){const a=normal(xy[Math.max(0,i-1)],xy[i===0?1:i]),b=normal(xy[i],xy[Math.min(xy.length-1,i+1)])??a;if(!a||!b)return null;const n={x:a.x+b.x,y:a.y+b.y},d=n.x*b.x+n.y*b.y;if(d<.25)return null;result.push(p.geo({x:xy[i].x+offset*n.x/d,y:xy[i].y+offset*n.y/d}));}return sampledGeometry(result).error?null:result;
}
export function parallelSection(line:GeoPoint[],from:GeoPoint,to:GeoPoint):EditedGeometry|null{
 const a=parallelProjection(line,from),b=parallelProjection(line,to);if(!a||!b||Math.abs(a.along-b.along)<20)return null;const low=Math.min(a.along,b.along),high=Math.max(a.along,b.along),p=plane(line[0]);let total=0;const points=[a.along<b.along?a.point:b.point];
 for(let i=1;i<line.length;i++){const x=p.xy(line[i-1]),y=p.xy(line[i]);total+=Math.hypot(y.x-x.x,y.y-x.y);if(total>low+.001&&total<high-.001)points.push(line[i]);}
 points.push(a.along<b.along?b.point:a.point);if(a.along>b.along)points.reverse();if(sampledGeometry(points).error)return null;return {version:1,anchors:[points[0],points.at(-1)!],line:points};
}
