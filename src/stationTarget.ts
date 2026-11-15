import {alignmentLength, nearestOnAlignment, pointOnAlignment, stationPolygon, type GeoPoint} from "./alignment";
import {project, type Camera} from "./map";

export type StationLine = {id:string; points:GeoPoint[]; pathId?:number; invalid?:boolean};
export function stationTarget(lines:StationLine[], point:GeoPoint, lengthM:number, camera:Camera, scale:number, occupied:{lineId:string;position:number;lengthM:number}[]=[]) {
  if(!Number.isFinite(lengthM)||lengthM<40||lengthM>400)return {error:"駅の長さは2〜20両にしてください。",target:null};
  const cursor=project(point.latitude,point.longitude,camera);
  const candidates=lines.filter(line=>line.points.length>1).map(line=>{
    const nearest=nearestOnAlignment(line.points,point), p=project(nearest.point.latitude,nearest.point.longitude,camera);
    return {line,nearest,pixels:Math.hypot(cursor.x-p.x,cursor.y-p.y)*scale};
  }).sort((a,b)=>a.pixels-b.pixels);
  const target=candidates[0];
  if(!target || target.pixels>20) return {error:"駅は線路上に置いてください",target:null};
  const polygon=stationPolygon(target.line.points,target.nearest.position,lengthM,12);
  const half=lengthM/alignmentLength(target.line.points)/2;
  const position=Math.max(half,Math.min(1-half,target.nearest.position));
  const center=pointOnAlignment(target.line.points,position);
  return {target:{...target,position,center,polygon},error:occupied.some(existing=>existing.lineId===target.line.id&&Math.abs(existing.position-position)*alignmentLength(target.line.points)<(existing.lengthM+lengthM)/2)?"駅が重なっています":target.line.invalid?"この線路計画は敷設できません。線形を修正してください。":polygon.length<4?"この線路に収まるホーム長を指定してください。":""};
}


// Geographic coordinates share a local plane here; no screen/zoom tolerance is used.
export function stationContainsPoint(polygon:GeoPoint[], point:GeoPoint):boolean {
  let inside=false;
  for(let i=0,j=polygon.length-1;i<polygon.length;j=i++){
    const a=polygon[i],b=polygon[j];
    if((a.latitude>point.latitude)!==(b.latitude>point.latitude)&&point.longitude<(b.longitude-a.longitude)*(point.latitude-a.latitude)/(b.latitude-a.latitude)+a.longitude)inside=!inside;
  }
  return inside;
}
