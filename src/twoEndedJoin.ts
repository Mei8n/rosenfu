import {type GeoPoint} from './alignment';
import {plannedGeometry,sampledGeometry,type EditedGeometry} from './editedGeometry';
import {extensionJoinError} from './extensionJoin';

/** Fit only the newly appended span. Explicit samples preserve the shape through
 * save/load and undo; existing spans and anchors are never moved. */
export function twoEndedJoin(anchors:GeoPoint[],edited:EditedGeometry|undefined,previous:GeoPoint|undefined,inside:GeoPoint):EditedGeometry|undefined {
 if(anchors.length<2)return;
 const prefix=plannedGeometry(anchors.slice(0,-1),edited,previous);
 if(prefix.error||!prefix.points.length)return;
 const a=anchors.at(-2)!,b=anchors.at(-1)!;
 const rad=Math.PI/180,scale=6371000*Math.cos(a.latitude*rad),oy=Math.asinh(Math.tan(a.latitude*rad));
 const xy=(p:GeoPoint)=>({x:(p.longitude-a.longitude)*rad*scale,y:(Math.asinh(Math.tan(p.latitude*rad))-oy)*scale});
 const geo=(x:number,y:number):GeoPoint=>({longitude:a.longitude+x/scale/rad,latitude:Math.atan(Math.sinh(oy+y/scale))/rad});
 const end=xy(b),next=xy(inside),length=Math.hypot(end.x,end.y);
 if(!Number.isFinite(length)||length<.1)return;
 let startT=prefix.tangent;
 if(!startT&&previous){const p=xy(previous),n=Math.hypot(p.x,p.y);if(n)startT={x:-p.x/n,y:-p.y/n};}
 startT??={x:end.x/length,y:end.y/length};
 const n=Math.hypot(next.x-end.x,next.y-end.y);if(!n)return;
 const endT={x:(next.x-end.x)/n,y:(next.y-end.y)/n};
 // A monotone chord projection prevents a hidden loop or reverse-facing cusp.
 if(startT.x*end.x+startT.y*end.y<=0||endT.x*end.x+endT.y*end.y<=0)return;
 for(const factor of [1/3,.2,.5,.75]){
  const h=length*factor,c1={x:startT.x*h,y:startT.y*h},c2={x:end.x-endT.x*h,y:end.y-endT.y*h};
  // Certify the whole cubic, not just rendered samples. Derivative control
  // vectors bound speed by projection; Bernstein coefficients bound |v × a|.
  const velocity=(t:number)=>({x:3*(1-t)**2*c1.x+6*(1-t)*t*(c2.x-c1.x)+3*t*t*(end.x-c2.x),y:3*(1-t)**2*c1.y+6*(1-t)*t*(c2.y-c1.y)+3*t*t*(end.y-c2.y)});
  const crossAt=(t:number)=>{const v=velocity(t),ax=6*(1-t)*(c2.x-2*c1.x)+6*t*(end.x-2*c2.x+c1.x),ay=6*(1-t)*(c2.y-2*c1.y)+6*t*(end.y-2*c2.y+c1.y);return v.x*ay-v.y*ax;};
  const safeRadius=(lo:number,hi:number,depth:number):boolean=>{
   const mid=(lo+hi)/2,v0=velocity(lo),v2=velocity(hi),vm=velocity(mid),v1={x:2*vm.x-(v0.x+v2.x)/2,y:2*vm.y-(v0.y+v2.y)/2},norm=Math.hypot(vm.x,vm.y);
   if(norm<.001)return false;
   const lower=Math.min(...[v0,v1,v2].map(v=>(v.x*vm.x+v.y*vm.y)/norm));
   const k0=crossAt(lo),k2=crossAt(hi),k1=2*crossAt(mid)-(k0+k2)/2,upper=Math.max(Math.abs(k0),Math.abs(k1),Math.abs(k2));
   const forward=Math.min(...[v0,v1,v2].map(v=>v.x*end.x+v.y*end.y));
   if(lower>0&&forward>0&&(upper<1e-12||lower**3>=25*upper))return true;
   if(depth===0)return false;
   return safeRadius(lo,mid,depth-1)&&safeRadius(mid,hi,depth-1);
  };
  if(!safeRadius(0,1,12))continue;
  const count=Math.max(64,Math.ceil(length/2));if(count+prefix.points.length>4096)return;
  const span:GeoPoint[]=[a];let valid=true,lastX=0,lastY=0;
  for(let i=0;i<=count;i++){
   const t=i/count,u=1-t;
   const dx=3*u*u*c1.x+6*u*t*(c2.x-c1.x)+3*t*t*(end.x-c2.x),dy=3*u*u*c1.y+6*u*t*(c2.y-c1.y)+3*t*t*(end.y-c2.y);
   const ddx=6*u*(c2.x-2*c1.x)+6*t*(end.x-2*c2.x+c1.x),ddy=6*u*(c2.y-2*c1.y)+6*t*(end.y-2*c2.y+c1.y);
   const speed=Math.hypot(dx,dy),cross=Math.abs(dx*ddy-dy*ddx);
   if(speed<.001||dx*end.x+dy*end.y<=0||(cross>1e-9&&speed**3/cross<25)){valid=false;break;}
   if(i===0)continue;
   const x=3*u*u*t*c1.x+3*u*t*t*c2.x+t*t*t*end.x,y=3*u*u*t*c1.y+3*u*t*t*c2.y+t*t*t*end.y;
   if(i>1){const dot=lastX*(x-xy(span.at(-1)!).x)+lastY*(y-xy(span.at(-1)!).y);if(dot<=0){valid=false;break;}}
   const prev=xy(span.at(-1)!);lastX=x-prev.x;lastY=y-prev.y;
   span.push(i===count?b:geo(x,y));
  }
  if(!valid)continue;
  const line=[...prefix.points,...span.slice(1)];
  if(sampledGeometry(line).error||extensionJoinError(line,previous)||extensionJoinError([...span].reverse(),inside))continue;
  return {version:1,anchors:anchors.map(p=>({...p})),line};
 }
}
