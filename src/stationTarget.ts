import {alignmentLength, nearestOnAlignment, pointOnAlignment, stationPolygon, type GeoPoint} from "./alignment";
import {project, type Camera} from "./map";

export type StationLine = {id:string; points:GeoPoint[]; planId?:number; invalid?:boolean};
export function stationTarget(lines:StationLine[], point:GeoPoint, lengthM:number, camera:Camera, scale:number) {
  const cursor=project(point.latitude,point.longitude,camera);
  const candidates=lines.filter(line=>line.points.length>1).map(line=>{
    const nearest=nearestOnAlignment(line.points,point), p=project(nearest.point.latitude,nearest.point.longitude,camera);
    return {line,nearest,pixels:Math.hypot(cursor.x-p.x,cursor.y-p.y)*scale};
  }).sort((a,b)=>a.pixels-b.pixels);
  const target=candidates[0];
  if(!target || target.pixels>20) return {error:"線路の近くを選んでください。建設済み・計画中の線路に沿って配置します。",target:null};
  const polygon=stationPolygon(target.line.points,target.nearest.position,lengthM,12);
  const half=lengthM/alignmentLength(target.line.points)/2;
  const position=Math.max(half,Math.min(1-half,target.nearest.position));
  const center=pointOnAlignment(target.line.points,position);
  return {target:{...target,position,center,polygon},error:target.line.invalid?"この線路計画は敷設できません。線形を修正してください。":polygon.length<4?"この線路に収まるホーム長を指定してください。":""};
}
