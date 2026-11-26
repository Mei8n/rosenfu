import {type GeoPoint} from "./alignment";
import {stationContainsPoint} from "./stationTarget";
export type StationGroup={station_id:string;members:string[]};
export type StationFootprint={id:string;name:string;owner:string;lineId:string;polygon:GeoPoint[];planned:boolean};
export type StationGroupView={id:string;name:string;members:StationFootprint[]};
export function footprintsIntersect(a:GeoPoint[],b:GeoPoint[]):boolean {
 if(a.length<3||b.length<3)return false;
 if(a.some(p=>stationContainsPoint(b,p))||b.some(p=>stationContainsPoint(a,p)))return true;
 const side=(p:GeoPoint,q:GeoPoint,r:GeoPoint)=>(q.longitude-p.longitude)*(r.latitude-p.latitude)-(q.latitude-p.latitude)*(r.longitude-p.longitude);
 return a.some((p,i)=>b.some((r,j)=>{const q=a[(i+1)%a.length],s=b[(j+1)%b.length];return side(p,q,r)*side(p,q,s)<0&&side(r,s,p)*side(r,s,q)<0;}));
}
function order(a:StationFootprint,b:StationFootprint){return Number(a.planned)-Number(b.planned)||a.id.localeCompare(b.id,undefined,{numeric:true});}
/** Saved built groups are authoritative. Only a new planned footprint joins by intersection. */
export function stationGroupViews(footprints:StationFootprint[],saved:StationGroup[]=[]):StationGroupView[]{
 const parent=footprints.map((_,i)=>i);
 const root=(i:number):number=>{
  while(parent[i]!==i){parent[i]=parent[parent[i]];i=parent[i];}
  return i;
 };
 const join=(a:number,b:number)=>{parent[root(b)]=root(a);};
 const savedGroup=new Map(saved.flatMap(g=>g.members.map(id=>[id,g.station_id] as const)));
 // Saved membership is exact and owner-scoped. Built/built geometry never
 // creates a group, so there is no need to enumerate those pairs.
 const byOwner=new Map<string,Map<string,number>>();
 const planned:number[]=[];
 footprints.forEach((footprint,i)=>{
  if(footprint.planned)planned.push(i);
  const group=savedGroup.get(footprint.id);
  if(group===undefined)return;
  let byGroup=byOwner.get(footprint.owner);
  if(!byGroup){byGroup=new Map();byOwner.set(footprint.owner,byGroup);}
  const first=byGroup.get(group);
  if(first===undefined)byGroup.set(group,i);
  else join(first,i);
 });
 for(const i of planned){
  const a=footprints[i];
  footprints.forEach((b,j)=>{
   if(j===i||(b.planned&&j<i)||a.owner!==b.owner||a.lineId===b.lineId)return;
   if(footprintsIntersect(a.polygon,b.polygon))join(i,j);
  });
 }
 const groups=new Map<number,StationFootprint[]>();
 footprints.forEach((p,i)=>{
  const key=root(i),members=groups.get(key);
  if(members)members.push(p);
  else groups.set(key,[p]);
 });
 return [...groups.values()].map(members=>{members.sort(order);return {id:members[0].id,name:members[0].name,members};});
}
