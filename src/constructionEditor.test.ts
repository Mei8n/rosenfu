import {expect,test} from 'vitest';
import {constructionEditorReducer as reduce,constructionEditorState} from './constructionEditor';
import {emptyConstructionPlans,type PlannedPath} from './constructionPlans';
const a={latitude:35,longitude:139},b={latitude:35,longitude:139.01};
const extension={alignmentId:'built',endpoint:'end' as const};
const path:PlannedPath={id:2,planId:1,name:'',points:[a,b],redo:[],line:[a,b],extension,trackType:'normal',designSpeedKmh:120,roadFollowing:false,anchorLevels:['ground','elevated'],destination:{alignmentId:'other',endpoint:'start'}};
test('start, append and finish change points, heights and destinations together',()=>{
 const initial=constructionEditorState();
 const started=reduce(initial,{type:'START_PATH',point:a,level:'ground',extension});
 const next=reduce(started,{type:'ADD_POINT',point:b,anchorLevels:['ground','elevated'],startLevel:'ground',destination:path.destination});
 expect(next).toMatchObject({draftPoints:[a,b],redoPoints:[],draftAnchorLevels:['ground','elevated'],draftFinished:true,draftExtension:extension,draftDestination:path.destination,draftRedoDestination:undefined});
 expect(initial.draftPoints).toEqual([]);
 expect(next.parkedPaths).toBe(initial.parkedPaths);
 expect(reduce(next,{type:'FINISH_PATH'})).toBe(next);
});
test('switching paths preserves completed destinations and parks the previous sampled geometry',()=>{
 const initial={...constructionEditorState(),parkedPaths:[path]};
 const current={...path,id:1,points:[a],line:[a],destination:undefined};
 const next=reduce(initial,{type:'SELECT_PATH',target:path,current,resume:true});
 expect(next.parkedPaths).toEqual([current]);
 expect(next.draftPoints).toBe(path.points);
 expect(next.draftAnchorLevels).toBe(path.anchorLevels);
 expect(next.draftFinished).toBe(true);
 const fresh=reduce(next,{type:'NEW_PATH',id:3,current:path});
 expect(fresh).toMatchObject({activePathId:3,draftPoints:[],redoPoints:[],redoAnchorLevels:[],draftExtension:null,draftDestination:undefined,draftFinished:false});
});
test('undo and redo apply journal snapshots, preserving unspecified height and extension metadata',()=>{
 const initial={...constructionEditorState(),draftPoints:[a,b],draftExtension:extension,draftAnchorLevels:['ground','elevated'] as const};
 const started={...initial,draftAnchorLevels:[...initial.draftAnchorLevels]};
 const undone=reduce(started,{type:'UNDO_POINT',id:1,edit:{points:[a],redo:[b],redoDestination:path.destination}});
 expect(undone.draftExtension).toBe(extension);
 expect(undone.draftAnchorLevels).toBe(started.draftAnchorLevels);
 expect(undone.draftRedoDestination).toEqual(path.destination);
 const restored=reduce(undone,{type:'REDO_POINT',id:1,edit:{points:[a,b],redo:[],destination:path.destination}});
 expect(restored.draftPoints).toEqual([a,b]);
 expect(restored.draftRedoDestination).toBeUndefined();
});
test('load restores exact arrays while clear affects only the active plan',()=>{
 const saved={...emptyConstructionPlans(),parkedPaths:[path,{...path,id:3,planId:2}],draftPoints:[a],draftExtension:extension};
 const loaded=reduce(constructionEditorState(),{type:'RESTORE_PLANS',plans:saved});
 expect(loaded.draftPoints).toBe(saved.draftPoints);
 expect(loaded.parkedPaths).toBe(saved.parkedPaths);
 const cleared=reduce(loaded,{type:'CLEAR_PLAN'});
 expect(cleared.parkedPaths).toEqual([saved.parkedPaths[1]]);
 expect(cleared.draftPoints).toEqual([]);
 expect(cleared.draftExtension).toBeNull();
});
