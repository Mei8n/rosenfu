import {expect,test} from 'vitest';
import {emptyConstructionPlans,recoverLegacyRedoPlans,type PlannedPath} from './constructionPlans';
const a={latitude:35,longitude:139},b={latitude:35,longitude:139.01},c={latitude:35,longitude:139.02};
const geometry={version:1 as const,anchors:[a,b,c],line:[a,{latitude:35,longitude:139.005},b,c]};
test.each([0,1,2])('recover legacy full and partial point Undo (%s points left) without inventing history',count=>{
 const source={...emptyConstructionPlans(),draftPoints:[a,b,c].slice(0,count),redoPoints:[a,b,c].slice(count).reverse(),draftEditedGeometry:geometry,draftAnchorLevels:['ground','elevated','ground'].slice(0,count) as ('ground'|'elevated')[],redoAnchorLevels:['ground','elevated','ground'].slice(count).reverse() as ('ground'|'elevated')[],plannedStations:[{id:'planned:1',pathId:1,planId:1,name:'駅',point:b,lengthM:100}]};
 const before=structuredClone(source),result=recoverLegacyRedoPlans(source);
 expect(result.recoveredPathIds).toEqual([1]);expect(result.unresolvedPathIds).toEqual([]);
 expect(result.plans.draftPoints).toEqual([a,b,c]);expect(result.plans.draftEditedGeometry).toEqual(geometry);
 expect(result.plans.draftAnchorLevels).toEqual(['ground','elevated','ground']);
 expect(result.plans.redoPoints).toEqual([]);expect(result.plans.redoAnchorLevels).toEqual([]);
 expect(result.plans.plannedStations).toEqual(source.plannedStations);expect(source).toEqual(before);
 expect(recoverLegacyRedoPlans(result.plans).recoveredPathIds).toEqual([]);
});
test('parked sampled geometry and restored endpoint keep path and station identities',()=>{
 const destination={alignmentId:'10',endpoint:'start' as const};
 const path:PlannedPath={id:4,planId:2,name:'parked',points:[a],redo:[c,b],line:[a],editedGeometry:geometry,extension:null,redoDestination:destination,trackType:'normal',designSpeedKmh:120,roadFollowing:false};
 const source={...emptyConstructionPlans(),plans:[{id:1,name:''},{id:2,name:'parked plan'}],parkedPaths:[path]};
 const result=recoverLegacyRedoPlans(source);
 expect(result.recoveredPathIds).toEqual([4]);expect(result.plans.parkedPaths[0]).toMatchObject({id:4,planId:2,points:[a,b,c],line:geometry.line,redo:[],destination});
 expect(result.plans.parkedPaths[0].redoDestination).toBeUndefined();expect(source.parkedPaths[0]).toBe(path);
});
test('missing extension context and mismatched explicit geometry retain all original recovery data',()=>{
 const source={...emptyConstructionPlans(),redoPoints:[b,a],draftExtension:{alignmentId:'10',endpoint:'end' as const}};
 expect(recoverLegacyRedoPlans(source)).toEqual({plans:source,recoveredPathIds:[],unresolvedPathIds:[1]});
 const valid=recoverLegacyRedoPlans(source,()=>({latitude:35,longitude:138.99}));expect(valid.recoveredPathIds).toEqual([1]);
 const bad={...emptyConstructionPlans(),redoPoints:[b,a],draftEditedGeometry:{...geometry,anchors:[c,b]}};
 expect(recoverLegacyRedoPlans(bad)).toEqual({plans:bad,recoveredPathIds:[],unresolvedPathIds:[1]});
});
test('active endpoint restoration and default heights do not construct anything',()=>{
 const destination={alignmentId:'20',endpoint:'start' as const};
 const source={...emptyConstructionPlans(),redoPoints:[b,a],startLevel:'elevated' as const,draftRedoDestination:destination};
 const result=recoverLegacyRedoPlans(source).plans;
 expect(result.draftDestination).toEqual(destination);expect(result.draftRedoDestination).toBeUndefined();expect(result.draftAnchorLevels).toEqual(['elevated','ground']);expect(result.draftFinished).toBe(true);
});
