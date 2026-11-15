import {plannedGeometry,type EditedGeometry} from './editedGeometry';
import {anchorIndices} from './deletionSections';
import type {ConstructionPlans,Extension,GeoPoint} from './constructionPlans';
export type PlannedDeleteTarget={kind:'plan-path';pathId:number}|{kind:'plan-anchor';pathId:number;index:number}|{kind:'plan-station';stationId:string};
/** Explicit individual deletion never silently drops a dependent planned station. */
export function removePlannedTarget(value:ConstructionPlans,target:PlannedDeleteTarget,previous:(extension:Extension|null)=>GeoPoint|undefined=()=>undefined):ConstructionPlans{
 if(target.kind==='plan-station'){
  if(!value.plannedStations.some(station=>station.id===target.stationId))throw Error('削除する駅計画が見つかりません');
  return {...value,plannedStations:value.plannedStations.filter(station=>station.id!==target.stationId)};
 }
 const active=target.pathId===value.activePathId;
 const path=active?{points:value.draftPoints,editedGeometry:value.draftEditedGeometry,extension:value.draftExtension,destination:value.draftDestination,reference:undefined,roadFollowing:value.roadFollowing}:value.parkedPaths.find(path=>path.id===target.pathId);
 if(!path||!path.points.length)throw Error('削除する計画線路が見つかりません');
 if(value.plannedStations.some(station=>station.pathId===target.pathId))throw Error('計画駅が使用中です。先に駅計画を削除してください');
 let points:GeoPoint[]=[],extension=path.extension,destination=path.destination,editedGeometry:EditedGeometry|undefined;
 if(target.kind==='plan-anchor'){
  if(!Number.isInteger(target.index)||target.index<0||target.index>=path.points.length)throw Error('削除するアンカーが見つかりません');
  if(path.reference||path.roadFollowing)throw Error('参照線路のアンカーは個別に削除できません');
  points=path.points.filter((_,index)=>index!==target.index);
  if(target.index===0)extension=null;
  if(target.index===path.points.length-1)destination=undefined;
  if(points.length>=2){
   const original=plannedGeometry(path.points,path.editedGeometry,previous(path.extension));
   if(original.error)throw Error(original.error);
   const indices=anchorIndices(path.points,original.points);
   if(!indices)throw Error('アンカー境界を特定できません');
   const start=target.index===0?indices[1]:0,end=target.index===path.points.length-1?indices.at(-2)!:original.points.length-1;
   // Removing an interior control point merges its two intervals, not the drawn shape.
   // Endpoint removal trims only its adjacent interval; custom curves elsewhere survive.
   editedGeometry={version:1,anchors:points,line:original.points.slice(start,end+1)};
  }
 }
 const geometry=plannedGeometry(points,editedGeometry,previous(extension));
 if(geometry.error)throw Error(geometry.error);
 if(active)return {...value,draftPoints:points,draftEditedGeometry:editedGeometry,redoPoints:[],draftExtension:points.length?extension:null,draftDestination:points.length>1?destination:undefined,draftRedoDestination:undefined,draftFinished:points.length>0};
 return {...value,parkedPaths:points.length?value.parkedPaths.map(item=>item.id===target.pathId?{...item,points,line:geometry.points,editedGeometry,redo:[],extension,destination:points.length>1?destination:undefined,redoDestination:undefined}:item):value.parkedPaths.filter(item=>item.id!==target.pathId)};
}
