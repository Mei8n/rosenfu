import {describe,it,expect} from 'vitest';
import {infrastructureSections,displayInfrastructureSections,endpointInfrastructure,schematicTrackPaths,LEVEL_LABELS} from './sectionInfrastructure';
import {removeConstructionPlan,emptyConstructionPlans,constructionPlanFingerprint} from './constructionPlans';
describe('physical section infrastructure',()=>{
 const points=[{latitude:35,longitude:139},{latitude:35,longitude:139.01},{latitude:35,longitude:139.02}];
 it('maps each interval without replacing geometry and preserves seven levels',()=>{
  const a={points,anchor_indices:[0,1,2],infrastructure:[{track_count:2 as const,level:'elevated_3' as const},{track_count:4 as const,level:'underground_2' as const}]};
  expect(infrastructureSections(a).map(s=>s.points)).toEqual([points.slice(0,2),points.slice(1)]);
  expect(endpointInfrastructure(a,'end')).toEqual(a.infrastructure[1]);
  expect(Object.keys(LEVEL_LABELS)).toHaveLength(7);
 });
 it('draws distinct schematic tracks, not duplicated alignment identities',()=>{
  for(const count of [1,2,4] as const){const paths=schematicTrackPaths([{x:0,y:0},{x:100,y:0}],count);expect(new Set(paths).size).toBe(count);expect(paths.every(p=>!p.includes('NaN'))).toBe(true);}
 });
 it('retains a parked path configuration on plan switch/removal',()=>{
  const base=emptyConstructionPlans();base.plans=[{id:1,name:'a'},{id:2,name:'b'}];
  base.parkedPaths=[{id:2,planId:2,name:'b',points,line:points,redo:[],extension:null,trackType:'normal',designSpeedKmh:120,roadFollowing:false,infrastructure:{track_count:4,level:'underground_3'}}];
  expect(removeConstructionPlan(base,1).infrastructure).toEqual({track_count:4,level:'underground_3'});
 });
});

it('normalizes struct key order and default infrastructure for clean saves',()=>{
 const empty=emptyConstructionPlans();
 expect(constructionPlanFingerprint({...empty,infrastructure:{track_count:1,level:'ground'}})).toBe(constructionPlanFingerprint(empty));
 const changed={...empty,infrastructure:{track_count:2 as const,level:'elevated' as const}};
 expect(constructionPlanFingerprint({infrastructure:changed.infrastructure,...empty})).toBe(constructionPlanFingerprint(changed));
 expect(constructionPlanFingerprint(changed)).not.toBe(constructionPlanFingerprint(empty));
});

it('keeps continuous same-attribute rendering without deleting physical boundaries',()=>{
 const points=[{latitude:35,longitude:139},{latitude:35,longitude:139.01},{latitude:35,longitude:139.02}];
 const a={points,anchor_indices:[0,1,2]};
 expect(displayInfrastructureSections(a)).toHaveLength(1);
 expect(displayInfrastructureSections(a)[0].points).toEqual(points);
 expect(infrastructureSections(a)).toHaveLength(2);
 expect(displayInfrastructureSections({...a,infrastructure:[{track_count:1,level:'ground'},{track_count:2,level:'ground'}]})).toHaveLength(2);
});
