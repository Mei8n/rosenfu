import type {EditedGeometry} from "./editedGeometry";
import type {ReferenceBuild} from "./referenceCopy";
import type {TrackType} from "./TrackTypeControls";
export type GeoPoint={latitude:number;longitude:number};
export type Extension={alignmentId:string;endpoint:"start"|"end"};
export type PlannedPath={destination?:Extension;redoDestination?:Extension;editedGeometry?:EditedGeometry;reference?:ReferenceBuild;extension:Extension|null;id:number;planId:number;name:string;points:GeoPoint[];redo:GeoPoint[];line:GeoPoint[];designSpeedKmh:number;trackType:TrackType;roadFollowing:boolean};
export type PlannedStation={id:string;pathId?:number;alignmentId?:string;planId?:number;name:string;point:GeoPoint;lengthM:number};
/** Saved editor data, distinct from built infrastructure. Hover and unconfirmed reference picks are transient. */
export type ConstructionPlans={version:1;plans:{id:number;name:string}[];parkedPaths:PlannedPath[];activePathId:number;activePlanId:number;draftPoints:GeoPoint[];draftEditedGeometry?:EditedGeometry;redoPoints:GeoPoint[];draftExtension:Extension|null;draftDestination?:Extension;draftRedoDestination?:Extension;trackType:TrackType;designSpeedKmh:number;roadFollowing:boolean;plannedStations:PlannedStation[];draftFinished:boolean};
export function emptyConstructionPlans():ConstructionPlans{return {version:1,plans:[{id:1,name:""}],parkedPaths:[],activePathId:1,activePlanId:1,draftPoints:[],redoPoints:[],draftExtension:null,trackType:"normal",designSpeedKmh:120,roadFollowing:false,plannedStations:[],draftFinished:false};}
export function nextConstructionIds(value:ConstructionPlans){return {plan:Math.max(...value.plans.map(p=>p.id),value.activePlanId)+1,path:Math.max(...value.parkedPaths.map(p=>p.id),value.activePathId)+1,station:Math.max(0,...value.plannedStations.map(s=>Number(s.id.replace(/^planned:/,""))).filter(Number.isSafeInteger))+1};}

/** Remove one explicit plan and its contents, keeping one blank editor only when none remain. */
export function removeConstructionPlan(value:ConstructionPlans,id:number):ConstructionPlans{
 const plans=value.plans.filter(plan=>plan.id!==id);
 const parkedPaths=value.parkedPaths.filter(path=>path.planId!==id);
 const plannedStations=value.plannedStations.filter(station=>(station.planId??(station.pathId===value.activePathId?value.activePlanId:value.parkedPaths.find(path=>path.id===station.pathId)?.planId))!==id);
 if(id!==value.activePlanId)return {...value,plans,parkedPaths,plannedStations};
 const activePlanId=plans[0]?.id??value.activePlanId;
 const path=parkedPaths.find(path=>path.planId===activePlanId&&!path.reference);
 return {...value,plans:plans.length?plans:[{id:activePlanId,name:""}],parkedPaths:parkedPaths.filter(p=>p!==path),plannedStations,activePlanId,
  activePathId:path?.id??nextConstructionIds(value).path,draftPoints:path?.points??[],redoPoints:path?.redo??[],draftEditedGeometry:path?.editedGeometry,
  draftExtension:path?.extension??null,draftDestination:path?.destination,draftRedoDestination:path?.redoDestination,draftFinished:!!path,
  trackType:path?.trackType??value.trackType,designSpeedKmh:path?.designSpeedKmh??value.designSpeedKmh,roadFollowing:path?.roadFollowing??value.roadFollowing};
}
