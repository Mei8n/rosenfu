import {distanceM,type GeoPoint} from './alignment';
import {project,type Camera} from './map';
export type JunctionSource={alignmentId:string;segmentIndex:number;fraction:number;reverse?:boolean;expectedAlignment:{id:string;owner:string;points:GeoPoint[];anchor_indices?:number[]|null;infrastructure?:{level:string}[];vertical_profile?:unknown}};
/** Screen-space hit testing; preserve exact segment/fraction instead of snapping to an unrelated anchor. */
export function nearestJunctionSource(lines:JunctionSource['expectedAlignment'][],point:GeoPoint,camera:Camera,scale:number,owner:string|undefined,level:string){
 const cursor=project(point.latitude,point.longitude,camera);let best:{source:JunctionSource;point:GeoPoint;distance:number}|null=null;
 for(const line of lines){
  if(line.owner!==owner||line.vertical_profile)continue;
  const anchors=line.anchor_indices??[0,line.points.length-1];
  for(let i=0;i<line.points.length-1;i++){
   const section=Math.max(0,anchors.findIndex((end,index)=>index>0&&end>=i+1)-1);
   const infrastructure=line.infrastructure?.[section];if((infrastructure?.level??'ground')!==level)continue;
   const a=project(line.points[i].latitude,line.points[i].longitude,camera),b=project(line.points[i+1].latitude,line.points[i+1].longitude,camera),dx=b.x-a.x,dy=b.y-a.y;
   const fraction=Math.max(0,Math.min(1,((cursor.x-a.x)*dx+(cursor.y-a.y)*dy)/(dx*dx+dy*dy||1)));
   const distance=Math.hypot(cursor.x-a.x-fraction*dx,cursor.y-a.y-fraction*dy)*scale;
   const target={latitude:line.points[i].latitude+(line.points[i+1].latitude-line.points[i].latitude)*fraction,longitude:line.points[i].longitude+(line.points[i+1].longitude-line.points[i].longitude)*fraction};
   if(distance>8||distanceM(target,line.points[0])<.1||distanceM(target,line.points.at(-1)!)<.1)continue;
   if(!best||distance<best.distance)best={source:{alignmentId:line.id,segmentIndex:i,fraction,expectedAlignment:line},point:target,distance};
  }
 }return best;
}
export function junctionDirection(source:JunctionSource,start:GeoPoint,toward:GeoPoint):JunctionSource{
 const a=source.expectedAlignment.points[source.segmentIndex],b=source.expectedAlignment.points[source.segmentIndex+1],cos=Math.cos(start.latitude*Math.PI/180);
 return {...source,reverse:(toward.longitude-start.longitude)*(b.longitude-a.longitude)*cos*cos+(toward.latitude-start.latitude)*(b.latitude-a.latitude)<0};
}
export function junctionPrevious(source:JunctionSource,start:GeoPoint):GeoPoint{
 const a=source.expectedAlignment.points[source.segmentIndex],b=source.expectedAlignment.points[source.segmentIndex+1],sign=source.reverse?-1:1;
 return {latitude:start.latitude-(b.latitude-a.latitude)*sign,longitude:start.longitude-(b.longitude-a.longitude)*sign};
}
