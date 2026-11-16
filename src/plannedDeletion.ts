import {plannedGeometry,type EditedGeometry} from './editedGeometry';
import {anchorIndices} from './deletionSections';
import {nextConstructionIds,type ConstructionPlans,type PlannedPath,type Extension,type GeoPoint} from './constructionPlans';
import {alignmentLength,nearestOnAlignment} from './alignment';
export type PlannedDeleteTarget={kind:'plan-path';pathId:number}|{kind:'plan-section';pathId:number;sectionIndex:number}|{kind:'plan-anchor';pathId:number;index:number}|{kind:'plan-station';stationId:string};
/** Explicit individual deletion never silently drops a dependent planned station. */
export function removePlannedTarget(value:ConstructionPlans,target:PlannedDeleteTarget,previous:(extension:Extension|null)=>GeoPoint|undefined=()=>undefined):ConstructionPlans{
 if(target.kind==='plan-station'){
  if(!value.plannedStations.some(station=>station.id===target.stationId))throw Error('削除する駅計画が見つかりません');
  return {...value,plannedStations:value.plannedStations.filter(station=>station.id!==target.stationId)};
 }
 const active=target.pathId===value.activePathId;
 const path=active?{anchorLevels:value.draftAnchorLevels,points:value.draftPoints,editedGeometry:value.draftEditedGeometry,extension:value.draftExtension,destination:value.draftDestination,reference:undefined,ofmPath:undefined,roadFollowing:value.roadFollowing}:value.parkedPaths.find(path=>path.id===target.pathId);
 if(!path||!path.points.length)throw Error('削除する計画線路が見つかりません');
 if(target.kind==='plan-section')return removePlannedSection(value,target,previous);
 if(value.plannedStations.some(station=>station.pathId===target.pathId))throw Error('計画駅が使用中です。先に駅計画を削除してください');
 let levels=path.anchorLevels,points:GeoPoint[]=[],extension=path.extension,destination=path.destination,editedGeometry:EditedGeometry|undefined;
 if(target.kind==='plan-anchor'){
  if(!Number.isInteger(target.index)||target.index<0||target.index>=path.points.length)throw Error('削除するアンカーが見つかりません');
  if(path.reference||path.ofmPath||path.roadFollowing)throw Error('参照線路のアンカーは個別に削除できません');
  if(levels&&target.index>0&&target.index<levels.length-1&&new Set(levels.slice(target.index-1,target.index+2)).size>1)throw Error("勾配が変化するアンカーの削除は未対応です");
  levels=levels?.filter((_,index)=>index!==target.index);
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
 if(points.length>1&&!extension&&destination){
  const line=plannedGeometry(points,editedGeometry).points.reverse();
  points.reverse();levels?.reverse();extension=destination;destination=undefined;editedGeometry={version:1,anchors:points,line};
 }
 const geometry=plannedGeometry(points,editedGeometry,previous(extension));
 if(geometry.error)throw Error(geometry.error);
 if(active)return {...value,draftAnchorLevels:points.length?levels:undefined,redoAnchorLevels:undefined,draftPoints:points,draftEditedGeometry:editedGeometry,redoPoints:[],draftExtension:points.length?extension:null,draftDestination:points.length>1?destination:undefined,draftRedoDestination:undefined,draftFinished:points.length>0};
 return {...value,parkedPaths:points.length?value.parkedPaths.map(item=>item.id===target.pathId?{...item,anchorLevels:levels,redoAnchorLevels:undefined,points,line:geometry.points,editedGeometry,redo:[],extension,destination:points.length>1?destination:undefined,redoDestination:undefined}:item):value.parkedPaths.filter(item=>item.id!==target.pathId)};
}

/** Cut an anchor interval without re-fitting surviving curves or moving station centers. */
function removePlannedSection(value:ConstructionPlans,target:Extract<PlannedDeleteTarget,{kind:'plan-section'}>,previous:(extension:Extension|null)=>GeoPoint|undefined):ConstructionPlans{
 const active=target.pathId===value.activePathId;
 const path:PlannedPath|undefined=active?{anchorLevels:value.draftAnchorLevels,startLevel:value.startLevel,id:value.activePathId,planId:value.activePlanId,name:'',points:value.draftPoints,line:[],redo:[],extension:value.draftExtension,destination:value.draftDestination,editedGeometry:value.draftEditedGeometry,infrastructure:value.infrastructure,designSpeedKmh:value.designSpeedKmh,trackType:value.trackType,roadFollowing:value.roadFollowing}:value.parkedPaths.find(p=>p.id===target.pathId);
 if(!path)throw Error('削除する計画線路が見つかりません');
 // Reference paths lack editable anchor semantics. Never guess their segmentation.
 if(path.reference||path.ofmPath||path.roadFollowing)throw Error('参照線路は区間ごとに削除できません');
 const original=plannedGeometry(path.points,path.editedGeometry,previous(path.extension));
 const indices=anchorIndices(path.points,original.points),i=target.sectionIndex;
 if(!indices||!Number.isInteger(i)||i<0||i>=indices.length-1)throw Error('削除する区間が見つかりません');
 const ranges:[number,number][]=[];
 if(i>0)ranges.push([0,i]);
 if(i+1<path.points.length-1)ranges.push([i+1,path.points.length-1]);
 const pieces=ranges.map(([start,end],index):PlannedPath=>{
  const levels=path.anchorLevels?.slice(start,end+1),points=path.points.slice(start,end+1),line=original.points.slice(indices[start],indices[end]+1);
  let extension=start===0?path.extension:null,destination=end===path.points.length-1?path.destination:undefined;
  // Construction supports extension-at-start, not destination-only paths. Reorient
  // a surviving tail from its existing endpoint without altering its physical shape.
  if(!extension&&destination){points.reverse();levels?.reverse();line.reverse();extension=destination;destination=undefined;}
  return {...path,anchorLevels:levels,redoAnchorLevels:undefined,id:index===0?path.id:nextConstructionIds(value).path,points,line,editedGeometry:{version:1,anchors:points,line},redo:[],extension,destination,redoDestination:undefined};
 });
 const length=alignmentLength(original.points),cutStart=alignmentLength(original.points.slice(0,indices[i]+1)),cutEnd=alignmentLength(original.points.slice(0,indices[i+1]+1));
 const plannedStations=value.plannedStations.map(station=>{
  if(station.pathId!==path.id)return station;
  const nearest=nearestOnAlignment(original.points,station.point),position=nearest.position*length,half=station.lengthM/2;
  // A whole platform must survive on one side; center-only checks permit severed stations.
  if(!Number.isFinite(position)||nearest.distanceM>1||position-half< -1e-6||position+half>length+1e-6||!(position+half<=cutStart+1e-6||position-half>=cutEnd-1e-6))throw Error('削除する区間に計画駅があります。先に駅計画を削除してください');
  const piece=pieces[ranges.findIndex(([start,end])=>position<cutStart?start===0:end===path.points.length-1)];
  if(!piece)throw Error('計画駅が使用中です');
  return {...station,pathId:piece.id};
 });
 const parkedPaths=value.parkedPaths.filter(p=>p.id!==path.id);
 if(!active)return {...value,parkedPaths:[...parkedPaths,...pieces],plannedStations};
 const first=pieces[0];
 return {...value,parkedPaths:[...parkedPaths,...pieces.slice(1)],plannedStations,draftAnchorLevels:first?.anchorLevels,redoAnchorLevels:undefined,startLevel:first?.startLevel,draftPoints:first?.points??[],draftEditedGeometry:first?.editedGeometry,redoPoints:[],draftExtension:first?.extension??null,draftDestination:first?.destination,draftRedoDestination:undefined,draftFinished:!!first};
}
