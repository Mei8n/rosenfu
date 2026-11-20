import type {GeoPoint} from './alignment';
/** Preview only: preserve the existing sampled shape outside the two adjacent
 * anchor intervals. This is NOT a circular-arc or topology representation. */
export function anchorDragPreview(line:readonly GeoPoint[],anchors:readonly GeoPoint[],index:number,target:GeoPoint):{line:GeoPoint[];error:string|null}{
 const unchanged=()=>line.map(p=>({...p}));
 const fail=(error:string)=>({line:unchanged(),error});
 if(!Number.isInteger(index)||index<0||index>=anchors.length)return fail('アンカーが見つかりません');
 if(!Number.isFinite(target.latitude)||!Number.isFinite(target.longitude)||Math.abs(target.latitude)>80||Math.abs(target.longitude)>180)return fail('移動先が範囲外です');
 const first=Math.max(0,index-1),last=Math.min(anchors.length-1,index+1);
 const positions=anchors.map(a=>line.findIndex(p=>p.latitude===a.latitude&&p.longitude===a.longitude));
 if(positions.some((p,i)=>p<0||(i>0&&p<=positions[i-1])))return fail('元の線形とアンカーが一致しません');
 const R=6371000,D=Math.PI/180,origin=anchors[index],scale=R*Math.cos(origin.latitude*D),oy=Math.asinh(Math.tan(origin.latitude*D));
 const xy=(p:GeoPoint)=>({x:(p.longitude-origin.longitude)*D*scale,y:(Math.asinh(Math.tan(p.latitude*D))-oy)*scale});
 const geo=(x:number,y:number):GeoPoint=>({longitude:origin.longitude+x/scale/D,latitude:Math.atan(Math.sinh(oy+y/scale))/D});
 const delta=xy(target),smooth=(t:number)=>t*t*(3-2*t);
 const result=line.slice(0,positions[first]).map(p=>({...p}));
 for(let side=first;side<last;side++){
  const start=positions[side],end=positions[side+1],points=line.slice(start,end+1).map(xy),lengths=[0];
  for(let i=1;i<points.length;i++)lengths.push(lengths[i-1]+Math.hypot(points[i].x-points[i-1].x,points[i].y-points[i-1].y));
  const total=lengths.at(-1)!;
  if(total<.1)return fail('隣接する区間が短すぎます');
  for(let i=0;i<points.length-1;i++){
   const a=points[i],b=points[i+1],length=lengths[i+1]-lengths[i],count=Math.max(1,Math.ceil(length/Math.max(2,total/256)));
   if(result.length+count>4096)return fail('編集する線形が長すぎます');
   for(let n=0;n<count;n++){
    const t=n/count,progress=(lengths[i]+length*t)/total,weight=side<index?smooth(progress):1-smooth(progress);
    result.push(n===0&&i===0?{...(side===index?target:anchors[first])}:geo(a.x+(b.x-a.x)*t+delta.x*weight,a.y+(b.y-a.y)*t+delta.y*weight));
   }
  }
 }
 result.push(...line.slice(positions[last]).map(p=>({...p})));
 if(index===anchors.length-1)result[result.length-1]={...target};
 return {line:result,error:null};
}
