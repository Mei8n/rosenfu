import {assertNever} from './assertNever';
import {emptyConstructionPlans,type ConstructionPlans,type PlannedPath,type GeoPoint,type Extension} from './constructionPlans';
import {plannedGeometry,type EditedGeometry} from './editedGeometry';
import type {JunctionSource} from './junctionSelection';
import type {SectionInfrastructure} from './sectionInfrastructure';

/** Editor transitions only. The existing edit journal owns Undo/Redo boundaries;
 * pointer previews, native requests and session histories never enter this state. */
export type ConstructionEditorState=Pick<ConstructionPlans,'plans'|'parkedPaths'|'activePathId'|'activePlanId'|'draftPoints'|'redoPoints'|'draftFinished'|'draftExtension'|'draftDestination'|'draftRedoDestination'|'draftEditedGeometry'|'draftJunctionSource'|'draftAnchorLevels'|'startLevel'>&{redoAnchorLevels:NonNullable<ConstructionPlans['redoAnchorLevels']>};
export type PathEdit={points:GeoPoint[];geometry?:EditedGeometry;redo:GeoPoint[];extension?:Extension|null;destination?:Extension;redoDestination?:Extension;anchorLevels?:SectionInfrastructure['level'][];redoAnchorLevels?:SectionInfrastructure['level'][];startLevel?:SectionInfrastructure['level'];infrastructure?:SectionInfrastructure};
export type ConstructionEditorAction=
 |{type:'RESTORE_PLANS';plans:ConstructionPlans}
 |{type:'NEW_PATH';id:number;current:PlannedPath|null}
 |{type:'NEW_PLAN';id:number}
 |{type:'SELECT_EMPTY_PLAN';id:number}
 |{type:'RENAME_PLAN';name:string}
 |{type:'SELECT_PATH';target:PlannedPath;current:PlannedPath|null;resume:boolean}
 |{type:'CLEAR_PLAN'}
 |{type:'PARK_REFERENCE_PATH';path:PlannedPath}
 |{type:'START_PATH';point:GeoPoint;extension?:Extension;level:SectionInfrastructure['level'];junction?:JunctionSource}
 |{type:'CLEAR_CONNECTIONS'}
 |{type:'RESUME_PATH'}
 |{type:'FINISH_PATH'}
 |{type:'SET_PARALLEL_PATH';geometry:EditedGeometry}
 |{type:'ADD_POINT';point:GeoPoint;geometry?:EditedGeometry;destination?:Extension;anchorLevels:SectionInfrastructure['level'][];startLevel:SectionInfrastructure['level']}
 |{type:'APPLY_PATH_EDIT'|'UNDO_POINT'|'REDO_POINT';id:number;edit:PathEdit;previous?:GeoPoint};

export function constructionEditorState(plans:ConstructionPlans=emptyConstructionPlans()):ConstructionEditorState{
 const {plans:headers,parkedPaths,activePathId,activePlanId,draftPoints,redoPoints,draftFinished,draftExtension,draftDestination,draftRedoDestination,draftEditedGeometry,draftJunctionSource,draftAnchorLevels,redoAnchorLevels=[],startLevel}=plans;
 return {plans:headers,parkedPaths,activePathId,activePlanId,draftPoints,redoPoints,draftFinished,draftExtension,draftDestination,draftRedoDestination,draftEditedGeometry,draftJunctionSource,draftAnchorLevels,redoAnchorLevels,startLevel};
}
const clearedDraft:Pick<ConstructionEditorState,'draftPoints'|'redoPoints'|'draftFinished'|'draftExtension'|'draftDestination'|'draftRedoDestination'|'draftEditedGeometry'>={draftPoints:[],redoPoints:[],draftFinished:false,draftExtension:null,draftDestination:undefined,draftRedoDestination:undefined,draftEditedGeometry:undefined};
export function constructionEditorReducer(state:ConstructionEditorState,action:ConstructionEditorAction):ConstructionEditorState{
 switch(action.type){
  case 'RESTORE_PLANS':return constructionEditorState(action.plans);
  case 'NEW_PATH':return {...state,...clearedDraft,draftAnchorLevels:undefined,redoAnchorLevels:[],draftJunctionSource:undefined,startLevel:undefined,activePathId:action.id,parkedPaths:action.current?[...state.parkedPaths,action.current]:state.parkedPaths};
  case 'NEW_PLAN':return {...state,plans:[...state.plans,{id:action.id,name:''}],activePlanId:action.id};
  case 'SELECT_EMPTY_PLAN':return {...state,activePlanId:action.id};
  case 'RENAME_PLAN':return {...state,plans:state.plans.map(plan=>plan.id===state.activePlanId?{...plan,name:action.name}:plan)};
  case 'SELECT_PATH':{
   const path=action.target;
   return {...state,parkedPaths:[...state.parkedPaths.filter(p=>p.id!==path.id),...(action.current?[action.current]:[])],activePathId:path.id,activePlanId:path.planId,draftPoints:path.points,redoPoints:path.redo,draftFinished:!action.resume||!!path.destination,draftExtension:path.extension,draftDestination:path.destination,draftRedoDestination:path.redoDestination,draftEditedGeometry:path.editedGeometry,draftJunctionSource:path.junctionSource,draftAnchorLevels:path.anchorLevels,redoAnchorLevels:path.redoAnchorLevels??[],startLevel:path.startLevel};
  }
  case 'CLEAR_PLAN':return {...state,...clearedDraft,draftAnchorLevels:undefined,redoAnchorLevels:[],parkedPaths:state.parkedPaths.filter(path=>path.planId!==state.activePlanId)};
  case 'PARK_REFERENCE_PATH':return {...state,parkedPaths:[...state.parkedPaths,action.path]};
  case 'START_PATH':return {...state,draftPoints:[action.point],redoPoints:[],draftFinished:false,draftExtension:action.extension??null,draftDestination:undefined,draftRedoDestination:undefined,draftJunctionSource:action.junction,startLevel:action.level,draftAnchorLevels:[action.level],redoAnchorLevels:[]};
  case 'CLEAR_CONNECTIONS':return !state.draftJunctionSource&&!state.draftExtension&&!state.draftDestination&&!state.draftRedoDestination?state:{...state,draftJunctionSource:undefined,draftExtension:null,draftDestination:undefined,draftRedoDestination:undefined};
  case 'RESUME_PATH':return state.draftFinished?{...state,draftFinished:false}:state;
  case 'FINISH_PATH':return state.draftFinished?state:{...state,draftFinished:true};
  case 'SET_PARALLEL_PATH':return {...state,draftPoints:action.geometry.anchors,draftEditedGeometry:action.geometry,draftFinished:true,redoPoints:[]};
  case 'ADD_POINT':return {...state,draftPoints:[...state.draftPoints,action.point],redoPoints:[],draftEditedGeometry:action.geometry,draftDestination:action.destination,draftRedoDestination:undefined,draftFinished:state.draftFinished||!!action.destination,draftAnchorLevels:action.anchorLevels,redoAnchorLevels:[],startLevel:state.draftPoints.length?state.startLevel:action.startLevel};
  case 'APPLY_PATH_EDIT':case 'UNDO_POINT':case 'REDO_POINT':{
   const value=action.edit;
   if(action.id===state.activePathId)return {...state,draftPoints:value.points,redoPoints:value.redo,draftEditedGeometry:value.geometry,draftDestination:value.destination,draftRedoDestination:value.redoDestination,...('anchorLevels' in value?{draftAnchorLevels:value.anchorLevels}:{}),...('redoAnchorLevels' in value?{redoAnchorLevels:value.redoAnchorLevels??[]}:{}),...('startLevel' in value?{startLevel:value.startLevel}:{}),...('extension' in value?{draftExtension:value.extension??null}:{})};
   return {...state,parkedPaths:state.parkedPaths.map(path=>path.id===action.id?{...path,anchorLevels:'anchorLevels' in value?value.anchorLevels:path.anchorLevels,redoAnchorLevels:'redoAnchorLevels' in value?value.redoAnchorLevels:path.redoAnchorLevels,startLevel:'startLevel' in value?value.startLevel:path.startLevel,infrastructure:value.infrastructure??path.infrastructure,extension:'extension' in value?value.extension??null:path.extension,destination:value.destination,redoDestination:value.redoDestination,points:value.points,editedGeometry:value.geometry,line:path.ofmPath?value.points:plannedGeometry(value.points,value.geometry,action.previous).points,redo:value.redo}:path)};
  }
  default:return assertNever(action);
 }
}
