import {distanceM, type GeoPoint} from './alignment';
import {circularAlignment, type CircularDraft} from './circularAlignment';
/** Explicit sampled shape: legacy anchor arrays are never reinterpreted on load. */
export type EditedGeometry={version:1;anchors:GeoPoint[];line:GeoPoint[]};
const equal=(a:GeoPoint,b:GeoPoint)=>a.latitude===b.latitude&&a.longitude===b.longitude;
export function sampledGeometry(line:GeoPoint[]):CircularDraft {
 const result:CircularDraft={points:line,arcs:[],guide:null,minimumRadiusM:Infinity,error:null,tangent:null};
 if(line.some(p=>!Number.isFinite(p.latitude)||!Number.isFinite(p.longitude)||p.latitude<20||p.latitude>46||p.longitude<122||p.longitude>154)||line.length>4096){result.error='線形が範囲外です';return result;}
 const scale=6371000*Math.cos((line[0]?.latitude??35)*Math.PI/180),rad=Math.PI/180;
 const xy=(p:GeoPoint)=>({x:p.longitude*rad*scale,y:Math.asinh(Math.tan(p.latitude*rad))*scale});
 for(let i=1;i<line.length;i++){
  if(distanceM(line[i-1],line[i])<.001){result.error='重複する点があります';return result;}
  if(i===line.length-1)break;
  const a=xy(line[i-1]),b=xy(line[i]),c=xy(line[i+1]),u={x:b.x-a.x,y:b.y-a.y},v={x:c.x-b.x,y:c.y-b.y};
  const cross=Math.abs(u.x*v.y-u.y*v.x),dot=u.x*v.x+u.y*v.y;
  if(dot<=0){result.error='線路が折り返しています';return result;}
  if(cross<1e-8)continue;
  const radius=Math.hypot(u.x,u.y)*Math.hypot(v.x,v.y)*Math.hypot(c.x-a.x,c.y-a.y)/(2*cross);
  result.minimumRadiusM=Math.min(result.minimumRadiusM,radius);
  result.arcs.push({segmentIndex:i-1,radiusM:radius,midpoint:line[i],startPointIndex:i-1,endPointIndex:i+1});
  if(radius<25)result.error='カーブの半径は25m以上必要です';
 }
 if(!result.error&&line.length>1){const a=xy(line.at(-2)!),b=xy(line.at(-1)!),length=Math.hypot(b.x-a.x,b.y-a.y);result.tangent={x:(b.x-a.x)/length,y:(b.y-a.y)/length};}
 return result;
}
export function plannedGeometry(anchors:GeoPoint[],edited?:EditedGeometry,previous?:GeoPoint):CircularDraft {
 if(!edited)return circularAlignment(anchors,previous);
 const bad=()=>({...sampledGeometry(edited.line),error:'編集した線形とアンカーが一致しません'});
 if(edited.version!==1||!edited.anchors.length||!edited.line.length)return bad();
 const prefix=Math.min(anchors.length,edited.anchors.length);
 if(anchors.slice(0,prefix).some((p,i)=>!equal(p,edited.anchors[i])))return bad();
 if(!anchors.length)return sampledGeometry([]);
 const end=edited.line.findIndex(p=>equal(p,anchors[Math.min(anchors.length,edited.anchors.length)-1]));
 if(end<0)return bad();
 const line=edited.line.slice(0,end+1);
 if(anchors.length>edited.anchors.length){
  const continuation=circularAlignment(anchors.slice(edited.anchors.length-1),line.at(-2)??previous);
  if(continuation.error)return continuation;
  line.push(...continuation.points.slice(1));
 }
 return sampledGeometry(line);
}
