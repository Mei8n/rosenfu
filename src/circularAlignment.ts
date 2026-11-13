import type { GeoPoint } from './alignment';
const R=6371000, DEG=Math.PI/180;
type XY={x:number;y:number};
export type CircleGuide={center:GeoPoint;edge:GeoPoint;radiusM:number};
export type CircularArc={segmentIndex:number;radiusM:number;midpoint:GeoPoint;startPointIndex:number;endPointIndex:number};
export type CircularDraft={points:GeoPoint[];arcs:CircularArc[];guide:CircleGuide|null;minimumRadiusM:number;error:string|null};
/** Provisional game-balance curve limit, not an actual train speed or engineering standard.
 * A straight segment (infinite radius) and invalid radii have no curve speed. */
export function provisionalCurveSpeed(radiusM:number):number|null {
 return Number.isFinite(radiusM)&&radiusM>0?Math.floor(3.6*Math.sqrt(radiusM)):null;
}
/** Incremental tangent arcs in a locally scaled Mercator plane. The first chord
 * establishes heading; adding a point never changes an earlier segment.
 * Metre radii are local approximations, not a surveyed track design. */
export function circularAlignment(anchors:GeoPoint[]):CircularDraft {
 const result:CircularDraft={points:[],arcs:[],guide:null,minimumRadiusM:Infinity,error:null};
 if(!anchors.length)return result;
 if(anchors.some(p=>!Number.isFinite(p.latitude)||!Number.isFinite(p.longitude)||Math.abs(p.latitude)>80||Math.abs(p.longitude)>180)){result.error='座標が範囲外です';return result;}
 const origin=anchors[0],scale=R*Math.cos(origin.latitude*DEG),oy=Math.asinh(Math.tan(origin.latitude*DEG));
 const xy=(p:GeoPoint):XY=>({x:(p.longitude-origin.longitude)*DEG*scale,y:(Math.asinh(Math.tan(p.latitude*DEG))-oy)*scale});
 const geo=(p:XY):GeoPoint=>({longitude:origin.longitude+p.x/scale/DEG,latitude:Math.atan(Math.sinh(oy+p.y/scale))/DEG});
 result.points.push({...origin});let tangent:XY|null=null;
 for(let i=1;i<anchors.length;i++){
  result.guide=null;
  const a=xy(anchors[i-1]),b=xy(anchors[i]),dx=b.x-a.x,dy=b.y-a.y,length=Math.hypot(dx,dy);
  if(length<.1){result.error='重複する点があります';break;}
  if(!tangent){tangent={x:dx/length,y:dy/length};result.points.push({...anchors[i]});continue;}
  const forward=dx*tangent.x+dy*tangent.y,lateral=-dx*tangent.y+dy*tangent.x;
  if(forward<=0){result.error='線路が折り返しています';break;}
  if(Math.abs(lateral)<Math.max(.001,length*1e-7)){result.points.push({...anchors[i]});tangent={x:dx/length,y:dy/length};continue;}
  const signedRadius=length*length/(2*lateral),radius=Math.abs(signedRadius);
  const center={x:a.x-tangent.y*signedRadius,y:a.y+tangent.x*signedRadius};
  result.guide={center:geo(center),edge:geo({x:center.x+radius,y:center.y}),radiusM:radius};
  result.minimumRadiusM=Math.min(result.minimumRadiusM,radius);
  if(radius<25)result.error='カーブの半径は25m以上必要です';
  const angle=2*Math.atan2(lateral,forward);
  // <=0.5m chord deviation, with explicit safety caps rather than silent truncation.
  const step=Math.min(Math.PI/36,2*Math.acos(Math.max(-1,1-.5/radius)));
  const count=Math.max(2,Math.ceil(Math.abs(angle)/step));
  if(count>256||result.points.length+count>4096){result.error='円弧が長すぎます。区間を分けてください';break;}
  const vx=a.x-center.x,vy=a.y-center.y;
  const startPointIndex=result.points.length-1;
  for(let n=1;n<=count;n++){
   const t=angle*n/count,c=Math.cos(t),s=Math.sin(t);
   result.points.push(n===count?{...anchors[i]}:geo({x:center.x+vx*c-vy*s,y:center.y+vx*s+vy*c}));
  }
  result.arcs.push({segmentIndex:i-1,radiusM:radius,
   midpoint:geo({x:center.x+vx*Math.cos(angle/2)-vy*Math.sin(angle/2),y:center.y+vx*Math.sin(angle/2)+vy*Math.cos(angle/2)}),
   startPointIndex,endPointIndex:result.points.length-1});
  const c=Math.cos(angle),s=Math.sin(angle);
  tangent={x:tangent.x*c-tangent.y*s,y:tangent.x*s+tangent.y*c};
  if(result.error)break;
 }
 return result;
}
