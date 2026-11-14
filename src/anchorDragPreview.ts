import type {GeoPoint} from './alignment';
/** Preview only: preserve the existing sampled shape outside the two adjacent
 * anchor intervals. This is NOT a circular-arc or topology representation. */
export function anchorDragPreview(line:readonly GeoPoint[],anchors:readonly GeoPoint[],index:number,target:GeoPoint):{line:GeoPoint[];error:string|null}{
 const unchanged=()=>line.map(p=>({...p}));
 const fail=(error:string)=>({line:unchanged(),error});
 if(!Number.isInteger(index)||index<=0||index>=anchors.length-1)return fail('Only interior anchors are supported by this preview');
 if(!Number.isFinite(target.latitude)||!Number.isFinite(target.longitude)||Math.abs(target.latitude)>80||Math.abs(target.longitude)>180)return fail('Invalid target');
 const positions=anchors.map(a=>line.findIndex(p=>p.latitude===a.latitude&&p.longitude===a.longitude));
 if(positions.some((p,i)=>p<0||(i>0&&p<=positions[i-1])))return fail('Anchor provenance is missing');
 const R=6371000,D=Math.PI/180,origin=anchors[index],scale=R*Math.cos(origin.latitude*D),oy=Math.asinh(Math.tan(origin.latitude*D));
 const xy=(p:GeoPoint)=>({x:(p.longitude-origin.longitude)*D*scale,y:(Math.asinh(Math.tan(p.latitude*D))-oy)*scale});
 const geo=(x:number,y:number):GeoPoint=>({longitude:origin.longitude+x/scale/D,latitude:Math.atan(Math.sinh(oy+y/scale))/D});
 const delta=xy(target),smooth=(t:number)=>t*t*(3-2*t);
 const result=line.slice(0,positions[index-1]).map(p=>({...p}));
 for(let side=index-1;side<=index;side++){
  const start=positions[side],end=positions[side+1],points=line.slice(start,end+1).map(xy),lengths=[0];
  for(let i=1;i<points.length;i++)lengths.push(lengths[i-1]+Math.hypot(points[i].x-points[i-1].x,points[i].y-points[i-1].y));
  const total=lengths.at(-1)!;
  if(total<.1)return fail('Degenerate adjacent interval');
  for(let i=0;i<points.length-1;i++){
   const a=points[i],b=points[i+1],length=lengths[i+1]-lengths[i],count=Math.max(1,Math.ceil(length/2));
   if(result.length+count>4096)return fail('Preview exceeds sample limit');
   for(let n=0;n<count;n++){
    const t=n/count,progress=(lengths[i]+length*t)/total,weight=side===index-1?smooth(progress):1-smooth(progress);
    result.push(n===0&&i===0?{...(side===index?target:anchors[index-1])}:geo(a.x+(b.x-a.x)*t+delta.x*weight,a.y+(b.y-a.y)*t+delta.y*weight));
   }
  }
 }
 result.push(...line.slice(positions[index+1]).map(p=>({...p})));
 return {line:result,error:null};
}
