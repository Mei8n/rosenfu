import type {GraphPoint} from './referenceGraph';
import {sampledGeometry} from './editedGeometry';
/** Round road corners with circular fillets rather than treating an angular road
 * polyline as track. No source code or tuning constants are taken from TW. */
export function fitRoadFollow(points:GraphPoint[]):GraphPoint[]{
 if(points.length<2)return points;
 const origin=points[0],sy=6371000*Math.PI/180,sx=sy*Math.cos(origin.latitude*Math.PI/180);
 const xy=points.map(p=>({x:(p.longitude-origin.longitude)*sx,y:(p.latitude-origin.latitude)*sy}));
 const corners=xy.map((b,i)=>{if(!i||i===xy.length-1)return null;const a=xy[i-1],c=xy[i+1],l=Math.hypot(b.x-a.x,b.y-a.y),r=Math.hypot(c.x-b.x,c.y-b.y);if(l<.01||r<.01)throw Error('道路の形状に重複があります');const u={x:(b.x-a.x)/l,y:(b.y-a.y)/l},v={x:(c.x-b.x)/r,y:(c.y-b.y)/r};const turn=Math.atan2(u.x*v.y-u.y*v.x,u.x*v.x+u.y*v.y);if(Math.abs(turn)<1e-5)return null;if(Math.abs(turn)>Math.PI*.95)throw Error('この曲がり角には線路を敷設できません');const radius=25,trim=radius*Math.tan(Math.abs(turn)/2);return {u,v,turn,trim,radius};});
 for(let i=1;i<xy.length;i++)if((corners[i-1]?.trim??0)+(corners[i]?.trim??0)>=Math.hypot(xy[i].x-xy[i-1].x,xy[i].y-xy[i-1].y)-.01)throw Error('カーブをつなぐための道路の長さが足りません');
 const out=[points[0]],geo=(p:{x:number;y:number})=>({longitude:origin.longitude+p.x/sx,latitude:origin.latitude+p.y/sy});
 for(let i=1;i<xy.length-1;i++){const c=corners[i],b=xy[i];if(!c){out.push(points[i]);continue;}const entry={x:b.x-c.u.x*c.trim,y:b.y-c.u.y*c.trim},side=Math.sign(c.turn),center={x:entry.x-c.u.y*c.radius*side,y:entry.y+c.u.x*c.radius*side},angle=Math.atan2(entry.y-center.y,entry.x-center.x),count=Math.ceil(Math.abs(c.turn)/(Math.PI/36));out.push(geo(entry));for(let n=1;n<=count;n++){const a=angle+c.turn*n/count;out.push(geo({x:center.x+c.radius*Math.cos(a),y:center.y+c.radius*Math.sin(a)}));}}
 out.push(points.at(-1)!);const error=sampledGeometry(out).error;if(error)throw Error(error);return out;
}
