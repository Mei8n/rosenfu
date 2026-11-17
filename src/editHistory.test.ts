import {expect,test} from 'vitest';
import {EditHistory,planEditKey,worldEditKey} from './editHistory';
import {emptyConstructionPlans} from './constructionPlans';
test('one chronological stream, completed steps only and new edit clears redo',()=>{const h=new EditHistory<number>();h.record(0,1,false);h.record(1,2,true);expect(h.peek(false)?.world).toBe(true);expect(h.peek(false)?.after).toBe(2); // failure: no complete
expect(h.undo).toHaveLength(2);h.complete(false);expect(h.peek(true)?.before).toBe(1);h.complete(false);expect(h.peek(true)?.world).toBe(false);h.complete(true);h.record(1,3,false);expect(h.redo).toEqual([]);h.clear();expect(h.undo).toEqual([]);expect(h.redo).toEqual([]);});
test('selection is not a plan edit; points and station changes are',()=>{const p=emptyConstructionPlans();expect(planEditKey(p)).toBe(planEditKey({...p,activePathId:2,draftFinished:true}));expect(planEditKey(p)).not.toBe(planEditKey({...p,draftPoints:[{latitude:35,longitude:139}]}));});
test('saving plans/view does not change world edit identity',()=>{expect(worldEditKey({next_id:'4',stations:[],view_state:{zoom:4},construction_plans:{}})).toBe(worldEditKey({next_id:'4',stations:[],view_state:{zoom:8},construction_plans:null}));});
test('parking the same path and cursor-only junction direction are not edits',()=>{
 const p={...emptyConstructionPlans(),draftPoints:[{latitude:35,longitude:139},{latitude:35.01,longitude:139}]};
 const parked={...emptyConstructionPlans(),activePathId:2,parkedPaths:[{id:p.activePathId,planId:p.activePlanId,name:'',points:p.draftPoints,line:p.draftPoints,redo:[],extension:null,trackType:p.trackType,designSpeedKmh:p.designSpeedKmh,infrastructure:{track_count:1 as const,level:'ground' as const},roadFollowing:false}]};
 expect(planEditKey(p)).toBe(planEditKey(parked));
 const junction={alignmentId:'3',segmentIndex:0,fraction:.5,reverse:false,expectedAlignment:{id:'3',owner:'2',points:p.draftPoints}};
 const a={...p,draftPoints:[p.draftPoints[0]],draftJunctionSource:junction};
 expect(planEditKey(a)).toBe(planEditKey({...a,draftJunctionSource:{...junction,reverse:true}}));
});
