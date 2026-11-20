import {describe,it,expect} from 'vitest';
import {emptyConstructionPlans} from './constructionPlans';
import {removePlannedTarget} from './plannedDeletion';
import {alignmentLength,pointOnAlignment} from './alignment';
import {destinationJoinError} from './destinationJoin';
import {plannedGeometry} from './editedGeometry';
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
  expect(removePlannedTarget(value,{kind:'plan-anchor',pathId:1,index:0}).draftExtension).toEqual(value.draftDestination);
  expect(removePlannedTarget(value,{kind:'plan-anchor',pathId:1,index:0}).draftDestination).toBeUndefined();
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

describe('planned anchor interval removal',()=>{
 const d={latitude:35.6,longitude:139.63};
 const bend={latitude:35.6001,longitude:139.605},bend2={latitude:35.6001,longitude:139.625};
 const value=()=>({...plan(),draftPoints:[a,b,c,d],draftEditedGeometry:{version:1 as const,anchors:[a,b,c,d],line:[a,bend,b,c,bend2,d]},draftExtension:{alignmentId:'source',endpoint:'end' as const},draftDestination:{alignmentId:'dest',endpoint:'start' as const}});
 it('splits the middle interval into exact surviving shapes in the same plan',()=>{
  const before=value(),after=removePlannedTarget(before,{kind:'plan-section',pathId:1,sectionIndex:1});
  expect(after.draftPoints).toEqual([a,b]);expect(after.draftEditedGeometry?.line).toEqual([a,bend,b]);
  expect(after.draftExtension).toEqual(before.draftExtension);expect(after.draftDestination).toBeUndefined();
  expect(after.parkedPaths).toHaveLength(1);expect(after.parkedPaths[0]).toMatchObject({id:2,planId:1,points:[d,c],line:[d,bend2,c],extension:before.draftDestination,redo:[]});
  expect(after.draftFinished).toBe(true);expect(before.draftPoints).toHaveLength(4);
  expect(JSON.parse(JSON.stringify(after)).parkedPaths[0].line).toEqual([d,bend2,c]);
  expect(after.parkedPaths[0].destination).toBeUndefined();
  expect(destinationJoinError(after.parkedPaths[0].line,after.parkedPaths[0].extension,after.parkedPaths[0].destination,[],[],undefined)).toBeNull();
  expect(plannedGeometry(after.parkedPaths[0].points,after.parkedPaths[0].editedGeometry).error).toBeNull();
 });
 it('trims first or last interval and clears only the removed endpoint link',()=>{
  const first=removePlannedTarget(value(),{kind:'plan-section',pathId:1,sectionIndex:0});
  expect(first.draftPoints).toEqual([d,c,b]);expect(first.draftExtension?.alignmentId).toBe('dest');expect(first.draftDestination).toBeUndefined();
  const last=removePlannedTarget(value(),{kind:'plan-section',pathId:1,sectionIndex:2});
  expect(last.draftPoints).toEqual([a,b,c]);expect(last.draftExtension?.alignmentId).toBe('source');expect(last.draftDestination).toBeUndefined();
 });
 it('removes sole interval without leaving orphan anchors, redo or endpoint metadata',()=>{
  const after=removePlannedTarget({...value(),draftPoints:[a,b],draftEditedGeometry:undefined,redoPoints:[c]},{kind:'plan-section',pathId:1,sectionIndex:0});
  expect(after.draftPoints).toEqual([]);expect(after.redoPoints).toEqual([]);expect(after.draftExtension).toBeNull();expect(after.draftDestination).toBeUndefined();expect(after.draftFinished).toBe(false);
 });
 it('retains station identity and position and reassigns only surviving suffix path',()=>{
  const left={id:'planned:1',pathId:1,name:'left',point:{latitude:35.6,longitude:139.605},lengthM:100};
  const right={...left,id:'planned:2',name:'right',point:{latitude:35.6,longitude:139.625}};
  const before={...value(),draftEditedGeometry:undefined,plannedStations:[left,right]};
  const after=removePlannedTarget(before,{kind:'plan-section',pathId:1,sectionIndex:1});
  expect(after.plannedStations).toEqual([left,{...right,pathId:2}]);
  for(const point of [b,c,{latitude:35.6,longitude:139.615}])expect(()=>removePlannedTarget({...before,plannedStations:[{...left,point}]},{kind:'plan-section',pathId:1,sectionIndex:1})).toThrow('計画駅');
 });
 it('splits parked paths without modifying active or unrelated paths and validates indices',()=>{
  const base=value(),parked={id:7,planId:2,name:'parked',points:base.draftPoints,line:base.draftEditedGeometry.line,editedGeometry:base.draftEditedGeometry,extension:null,redo:[],designSpeedKmh:120,trackType:'normal' as const,roadFollowing:false};
  const before={...plan(),parkedPaths:[parked]};
  const after=removePlannedTarget(before,{kind:'plan-section',pathId:7,sectionIndex:1});
  expect(after.draftPoints).toEqual(before.draftPoints);expect(after.parkedPaths.map(p=>p.id)).toEqual([7,8]);expect(after.parkedPaths.every(p=>p.planId===2)).toBe(true);
  expect(()=>removePlannedTarget(before,{kind:'plan-section',pathId:7,sectionIndex:3})).toThrow('区間');
 });
});

it('retains platforms touching surviving boundaries despite projection roundoff',()=>{
 const line=[a,b,c],length=alignmentLength(line),station={id:'planned:1',pathId:1,name:'edge',lengthM:100,point:pointOnAlignment(line,50/length)};
 expect(removePlannedTarget({...plan(),plannedStations:[station]},{kind:'plan-section',pathId:1,sectionIndex:1}).plannedStations[0]).toEqual(station);
 const end={...station,point:pointOnAlignment(line,1-50/length)};
 expect(removePlannedTarget({...plan(),plannedStations:[end]},{kind:'plan-section',pathId:1,sectionIndex:0}).plannedStations[0]).toEqual(end);
});
