import {describe,it,expect} from 'vitest';
import {emptyConstructionPlans} from './constructionPlans';
import {removePlannedTarget} from './plannedDeletion';
import {anchorIndices,deletionSections} from './deletionSections';
const a={latitude:35.6,longitude:139.6},b={latitude:35.6,longitude:139.61},c={latitude:35.6,longitude:139.62};
const plan=()=>({...emptyConstructionPlans(),draftPoints:[a,b,c],draftFinished:true});
describe('individual plan deletion',()=>{
 it('removes one anchor without resuming drawing, clears stale redo and preserves other plans',()=>{
  const value={...plan(),plans:[{id:1,name:'one'},{id:2,name:'two'}],redoPoints:[c]};
  const result=removePlannedTarget(value,{kind:'plan-anchor',pathId:1,index:1});
  expect(result.draftPoints).toEqual([a,c]);expect(result.draftFinished).toBe(true);expect(result.redoPoints).toEqual([]);expect(result.plans).toEqual(value.plans);expect(value.draftPoints).toHaveLength(3);
 });
 it('removes a singleton and permits deleting complete paths',()=>{
  expect(removePlannedTarget({...plan(),draftPoints:[a]},{kind:'plan-anchor',pathId:1,index:0}).draftPoints).toEqual([]);
  expect(removePlannedTarget(plan(),{kind:'plan-path',pathId:1}).draftPoints).toEqual([]);
 });
 it('clears only deleted endpoint references',()=>{
  const value={...plan(),draftExtension:{alignmentId:'3',endpoint:'end' as const},draftDestination:{alignmentId:'4',endpoint:'start' as const}};
  expect(removePlannedTarget(value,{kind:'plan-anchor',pathId:1,index:0}).draftExtension).toBeNull();
  expect(removePlannedTarget(value,{kind:'plan-anchor',pathId:1,index:2}).draftDestination).toBeUndefined();
 });
 it('protects station dependencies until station explicitly deleted',()=>{
  const value={...plan(),plannedStations:[{id:'planned:1',pathId:1,planId:1,name:'駅',point:b,lengthM:100}]};
  expect(()=>removePlannedTarget(value,{kind:'plan-anchor',pathId:1,index:1})).toThrow('計画駅');
  expect(()=>removePlannedTarget(value,{kind:'plan-path',pathId:1})).toThrow('計画駅');
  const removed=removePlannedTarget(value,{kind:'plan-station',stationId:'planned:1'});
  expect(removed.draftPoints).toEqual(value.draftPoints);expect(removePlannedTarget(removed,{kind:'plan-path',pathId:1}).draftPoints).toEqual([]);
 });
 it('does not remove unrelated parked paths or accept bad indices',()=>{
  const path={id:2,planId:2,name:'',points:[a,b],redo:[],line:[a,b],extension:null,trackType:'normal' as const,designSpeedKmh:120,roadFollowing:false};
  const value={...plan(),parkedPaths:[path]};
  const result=removePlannedTarget(value,{kind:'plan-anchor',pathId:2,index:1});
  expect(result.draftPoints).toEqual(value.draftPoints);expect(result.parkedPaths[0].points).toEqual([a]);
  expect(()=>removePlannedTarget(value,{kind:'plan-anchor',pathId:2,index:3})).toThrow('見つかりません');
  expect(removePlannedTarget(result,{kind:'plan-path',pathId:2}).parkedPaths).toEqual([]);
 });
});
it('only real persisted anchors divide built sections, legacy remains whole',()=>{
 expect(deletionSections({points:[a,b,c]})).toEqual([{index:0,points:[a,b,c]}]);
 expect(deletionSections({points:[a,b,c],anchor_indices:[0,1,2]})).toHaveLength(2);
 expect(deletionSections({points:[a,b,c],anchor_indices:[0,0,2]})).toEqual([]);
 expect(anchorIndices([a,c],[a,b,c])).toEqual([0,2]);expect(anchorIndices([c,a],[a,b,c])).toBeUndefined();
});
it('deleting an edited endpoint preserves the sampled surviving curve; interior removal only merges intervals',()=>{
 const bend={latitude:35.6001,longitude:139.605},bend2={latitude:35.6001,longitude:139.615};
 const line=[a,bend,b,bend2,c];
 const value={...plan(),draftEditedGeometry:{version:1 as const,anchors:[a,b,c],line}};
 const end=removePlannedTarget(value,{kind:'plan-anchor',pathId:1,index:2});
 expect(end.draftEditedGeometry?.line).toEqual([a,bend,b]);
 const start=removePlannedTarget(value,{kind:'plan-anchor',pathId:1,index:0});
 expect(start.draftEditedGeometry?.line).toEqual([b,bend2,c]);
 const middle=removePlannedTarget(value,{kind:'plan-anchor',pathId:1,index:1});
 expect(middle.draftEditedGeometry?.anchors).toEqual([a,c]);expect(middle.draftEditedGeometry?.line).toEqual(line);
});
