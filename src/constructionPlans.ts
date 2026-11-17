import type {OfmPathDescriptor} from './ofmPlannedReference';
import type {JunctionSource} from './junctionSelection';
import type {SectionInfrastructure} from './sectionInfrastructure';
import {plannedGeometry,type EditedGeometry} from "./editedGeometry";
import {anchorHeights} from "./anchorHeights";
import type {ReferenceBuild} from "./referenceCopy";
import type {TrackType} from "./TrackTypeControls";
export type GeoPoint={latitude:number;longitude:number};
export type Extension={alignmentId:string;endpoint:"start"|"end"};
export type PlannedPath={anchorLevels?:SectionInfrastructure["level"][];redoAnchorLevels?:SectionInfrastructure["level"][];junctionSource?:JunctionSource;startLevel?:SectionInfrastructure["level"];infrastructure?:SectionInfrastructure;destination?:Extension;redoDestination?:Extension;editedGeometry?:EditedGeometry;reference?:ReferenceBuild;ofmPath?:OfmPathDescriptor;extension:Extension|null;id:number;planId:number;name:string;points:GeoPoint[];redo:GeoPoint[];line:GeoPoint[];designSpeedKmh:number;trackType:TrackType;roadFollowing:boolean};
export type PlannedStation={id:string;pathId?:number;alignmentId?:string;planId?:number;name:string;point:GeoPoint;lengthM:number};
/** Saved editor data, distinct from built infrastructure. Hover and unconfirmed reference picks are transient. */
export type ConstructionPlans={draftAnchorLevels?:SectionInfrastructure["level"][];redoAnchorLevels?:SectionInfrastructure["level"][];draftJunctionSource?:JunctionSource;startLevel?:SectionInfrastructure["level"];infrastructure?:SectionInfrastructure;version:1;plans:{id:number;name:string}[];parkedPaths:PlannedPath[];activePathId:number;activePlanId:number;draftPoints:GeoPoint[];draftEditedGeometry?:EditedGeometry;redoPoints:GeoPoint[];draftExtension:Extension|null;draftDestination?:Extension;draftRedoDestination?:Extension;trackType:TrackType;designSpeedKmh:number;roadFollowing:boolean;plannedStations:PlannedStation[];draftFinished:boolean};
export function emptyConstructionPlans():ConstructionPlans{return {version:1,plans:[{id:1,name:""}],parkedPaths:[],activePathId:1,activePlanId:1,draftPoints:[],redoPoints:[],draftExtension:null,trackType:"normal",designSpeedKmh:120,roadFollowing:false,plannedStations:[],draftFinished:false};}
export function nextConstructionIds(value:ConstructionPlans){return {plan:Math.max(...value.plans.map(p=>p.id),value.activePlanId)+1,path:Math.max(...value.parkedPaths.map(p=>p.id),value.activePathId)+1,station:Math.max(0,...value.plannedStations.map(s=>Number(s.id.replace(/^planned:/,""))).filter(Number.isSafeInteger))+1};}

/** Remove one explicit plan and its contents, keeping one blank editor only when none remain. */
export function removeConstructionPlan(value:ConstructionPlans,id:number):ConstructionPlans{
 const plans=value.plans.filter(plan=>plan.id!==id);
 const parkedPaths=value.parkedPaths.filter(path=>path.planId!==id);
 const plannedStations=value.plannedStations.filter(station=>(station.planId??(station.pathId===value.activePathId?value.activePlanId:value.parkedPaths.find(path=>path.id===station.pathId)?.planId))!==id);
 if(id!==value.activePlanId)return {...value,plans,parkedPaths,plannedStations};
 const activePlanId=plans[0]?.id??value.activePlanId;
 const path=parkedPaths.find(path=>path.planId===activePlanId&&!path.reference&&!path.ofmPath);
 return {...value,plans:plans.length?plans:[{id:activePlanId,name:""}],parkedPaths:parkedPaths.filter(p=>p!==path),plannedStations,activePlanId,
  activePathId:path?.id??nextConstructionIds(value).path,draftPoints:path?.points??[],redoPoints:path?.redo??[],draftEditedGeometry:path?.editedGeometry,
  draftExtension:path?.extension??null,draftDestination:path?.destination,draftRedoDestination:path?.redoDestination,draftFinished:!!path,
  draftAnchorLevels:path?.anchorLevels,redoAnchorLevels:path?.redoAnchorLevels,draftJunctionSource:path?.junctionSource,startLevel:path?.startLevel,infrastructure:path?.infrastructure??value.infrastructure,trackType:path?.trackType??value.trackType,designSpeedKmh:path?.designSpeedKmh??value.designSpeedKmh,roadFollowing:path?.roadFollowing??value.roadFollowing};
}

/** Drop only empty plan headers at persistence boundaries, never in-progress anchors or undo data. */
export function compactEmptyPlans(value:ConstructionPlans):ConstructionPlans{
 const occupied=new Set(value.parkedPaths.filter(path=>path.points.length||path.redo.length||path.line.length||path.reference||path.ofmPath||path.editedGeometry||path.extension||path.destination||path.redoDestination).map(path=>path.planId));
 if(value.draftPoints.length||value.redoPoints.length||value.draftEditedGeometry||value.draftExtension||value.draftDestination||value.draftRedoDestination)occupied.add(value.activePlanId);
 for(const station of value.plannedStations){
  const owner=station.planId??(station.pathId===value.activePathId?value.activePlanId:value.parkedPaths.find(path=>path.id===station.pathId)?.planId);
  // Preserve ambiguous legacy data instead of guessing which empty header owns it.
  if(owner===undefined)return value;
  occupied.add(owner);
 }
 if(!occupied.size)return {...value,plans:[{id:value.activePlanId,name:""}]};
 let result=value;
 for(const plan of value.plans)if(!occupied.has(plan.id))result=removeConstructionPlan(result,plan.id);
 return result;
}

/** Stable comparison across Rust struct ordering and the compact default encoding. */
export function constructionPlanFingerprint(value:ConstructionPlans):string{
 return canonicalFingerprint(value);
}

/** Shared canonical encoding for document and World history comparisons. */
export function canonicalFingerprint(value:object):string{
 function canonical(item:unknown):unknown{
  if(Array.isArray(item))return item.map(canonical);
  if(item&&typeof item==='object'){
   return Object.fromEntries(Object.entries(item).filter(([key,value])=>value!==undefined&&!(key==='infrastructure'&&value&&typeof value==='object'&&'track_count' in value&&value.track_count===1&&'level' in value&&value.level==='ground')).sort(([a],[b])=>a.localeCompare(b)).map(([key,value])=>[key,canonical(value)]));
  }
  return item;
 }
 return JSON.stringify(canonical(value));
}

/** Recover legacy persisted point-Redo as visible, unbuilt plans, not session history.
 * Call at a load boundary only, never when restoring a current-session Undo snapshot.
 * Uncertain geometry is retained byte-for-byte for manual recovery, not discarded.
 */
export function recoverLegacyRedoPlans(value:ConstructionPlans,previousForExtension?:(extension:Extension)=>GeoPoint|undefined):{plans:ConstructionPlans;recoveredPathIds:number[];unresolvedPathIds:number[]}{
 const recoveredPathIds:number[]=[],unresolvedPathIds:number[]=[];
 function recover(id:number,points:GeoPoint[],redo:GeoPoint[],geometry:EditedGeometry|undefined,extension:Extension|null,levels:SectionInfrastructure['level'][]|undefined,redoLevels:SectionInfrastructure['level'][]|undefined,start:SectionInfrastructure['level']|undefined,end:SectionInfrastructure['level'],opaque=false){
  if(!redo.length)return null;
  const restored=[...points,...redo.slice().reverse()];
  const previous=extension?previousForExtension?.(extension):undefined;
  // Reference/OFM descriptors may bind their own point sequence. Do not invent
  // replacement provenance or silently detach it to make a recovery succeed.
  if(opaque||(extension&&!previous)||restored.some(p=>!Number.isFinite(p.latitude)||!Number.isFinite(p.longitude))||(levels&&levels.length!==points.length)||(redoLevels&&redoLevels.length!==redo.length)){
   unresolvedPathIds.push(id);return null;
  }
  const shape=plannedGeometry(restored,geometry,previous);
  if(shape.error==='編集した線形とアンカーが一致しません'||shape.error==='線形が範囲外です'){
   unresolvedPathIds.push(id);return null;
  }
  const heights=[...anchorHeights(points.length,levels,start,end),...(redoLevels??Array.from({length:redo.length},()=>end)).slice().reverse()];
  if(!points.length&&!redoLevels&&heights.length)heights[0]=start??end;
  recoveredPathIds.push(id);return {points:restored,line:shape.points,levels:heights};
 }
 const active=recover(value.activePathId,value.draftPoints,value.redoPoints,value.draftEditedGeometry,value.draftExtension,value.draftAnchorLevels,value.redoAnchorLevels,value.startLevel,value.infrastructure?.level??'ground',value.roadFollowing);
 const parkedPaths=value.parkedPaths.map(path=>{
  const result=recover(path.id,path.points,path.redo,path.editedGeometry,path.extension,path.anchorLevels,path.redoAnchorLevels,path.startLevel,path.infrastructure?.level??'ground',!!path.reference||!!path.ofmPath||path.roadFollowing);
  return result?{...path,points:result.points,line:result.line,anchorLevels:result.levels,redo:[],redoAnchorLevels:[],destination:path.redoDestination??path.destination,redoDestination:undefined}:path;
 });
 if(!recoveredPathIds.length)return {plans:value,recoveredPathIds,unresolvedPathIds};
 const plans:ConstructionPlans={...value,parkedPaths,...(active?{draftPoints:active.points,draftAnchorLevels:active.levels,redoPoints:[],redoAnchorLevels:[],draftDestination:value.draftRedoDestination??value.draftDestination,draftRedoDestination:undefined,draftFinished:active.points.length>=2}:{})};
 return {plans,recoveredPathIds,unresolvedPathIds};
}
