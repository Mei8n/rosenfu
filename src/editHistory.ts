import {canonicalFingerprint,type ConstructionPlans} from './constructionPlans';

/** Session-only history. A final operation discards history, never the live document. */
export class EditHistory<T> {
  undo: {before:T;after:T;world:boolean}[]=[];
  redo: {before:T;after:T;world:boolean}[]=[];
  clear(){this.undo=[];this.redo=[];}
  record(before:T,after:T,world:boolean){this.undo.push({before,after,world});this.undo=this.undo.slice(-50);this.redo=[];}
  peek(redo:boolean){return (redo?this.redo:this.undo).at(-1);}
  complete(redo:boolean){const source=redo?this.redo:this.undo,entry=source.pop();if(entry)(redo?this.undo:this.redo).push(entry);}
}
/** Selecting a tab/path or finishing a stroke is not a document edit. */
export function planEditKey(value:ConstructionPlans):string {
 const path={id:value.activePathId,planId:value.activePlanId,points:value.draftPoints,editedGeometry:value.draftEditedGeometry,extension:value.draftExtension,destination:value.draftDestination,anchorLevels:value.draftAnchorLevels,junctionSource:value.draftJunctionSource,startLevel:value.startLevel,infrastructure:value.infrastructure,trackType:value.trackType,designSpeedKmh:value.designSpeedKmh};
 const paths=[...value.parkedPaths.map(p=>({id:p.id,planId:p.planId,points:p.points,editedGeometry:p.editedGeometry,extension:p.extension,destination:p.destination,anchorLevels:p.anchorLevels,junctionSource:p.junctionSource,startLevel:p.startLevel,infrastructure:p.infrastructure,trackType:p.trackType,designSpeedKmh:p.designSpeedKmh,reference:p.reference,ofmPath:p.ofmPath})),path].filter(p=>p.points.length||('reference' in p&&p.reference)||('ofmPath' in p&&p.ofmPath)).sort((a,b)=>a.id-b.id);
 // A first junction anchor previews its outgoing direction under the cursor;
 // only the second committed anchor makes that direction a document edit.
 for(const path of paths)if(path.points.length<2&&path.junctionSource)path.junctionSource={...path.junctionSource,reverse:undefined};
 const occupied=new Set(paths.map(path=>path.planId));
 return canonicalFingerprint({plans:value.plans.filter(plan=>occupied.has(plan.id)||value.plannedStations.some(station=>station.planId===plan.id)),paths,stations:value.plannedStations});
}
export function worldEditKey(value:unknown):string {
 if(!value||typeof value!=='object')return '';
 const world={...value};
 if('construction_plans' in world)delete world.construction_plans;
 if('view_state' in world)delete world.view_state;
 return canonicalFingerprint(world);
}
export function sameWorldEdit(a:unknown,b:unknown):boolean{return a===b||worldEditKey(a)===worldEditKey(b);}
export const FINAL_WORLD_COMMANDS=new Set(['build_plan','build_plan_with_roads','build_alignment','build_alignment_with_stations','build_alignment_from_reference','place_station_on_alignment','add_station','update_station_placement','update_alignment_anchor_height','update_alignment_section_infrastructure','update_alignment_section_track_type']);
export const UNDO_WORLD_COMMANDS=new Set(['delete_alignment','delete_alignment_section','delete_alignment_sections','delete_station','rename_station','rename_company','rename_route','save_planned_route','create_service_path','update_service_path','replace_service_path','delete_service','duplicate_service','set_service_appearance','set_service_color','set_service_route_binding']);
