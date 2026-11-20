import {expect,test} from 'vitest';
import {emptyConstructionPlans,compactEmptyPlans,removeConstructionPlan} from './constructionPlans';
test('station-only and redo-only plans survive restoration',()=>{
 const base={...emptyConstructionPlans(),plans:[{id:1,name:''},{id:2,name:''}],activePlanId:2,redoPoints:[{latitude:35,longitude:139}],plannedStations:[{id:'planned:1',planId:1,alignmentId:'3',name:'駅',point:{latitude:35,longitude:139},lengthM:100}]};
 expect(base.plans).toHaveLength(2);
 const result=removeConstructionPlan(base,2);expect(result.plannedStations).toEqual(base.plannedStations);expect(result.activePlanId).toBe(1);expect(result.redoPoints).toEqual([]);
});

test('empty headers including name-only plans are omitted at persistence, not content',()=>{
 const base={...emptyConstructionPlans(),plans:[{id:1,name:'空の計画'},{id:2,name:'名前のみ'}],activePlanId:2};
 expect(compactEmptyPlans(base).plans).toEqual([{id:2,name:''}]);
 const withAnchor={...base,draftPoints:[{latitude:35,longitude:139}]};
 expect(compactEmptyPlans(withAnchor).plans).toEqual([{id:2,name:'名前のみ'}]);
 expect(compactEmptyPlans({...base,redoPoints:withAnchor.draftPoints}).plans).toHaveLength(1);
 const ambiguous={...base,plannedStations:[{id:'planned:1',name:'駅',point:withAnchor.draftPoints[0],lengthM:100}]};
 expect(compactEmptyPlans(ambiguous)).toBe(ambiguous);
 const stationOnly={...base,plannedStations:[{...ambiguous.plannedStations[0],planId:1,alignmentId:'3'}]};
 expect(compactEmptyPlans(stationOnly).plans).toEqual([{id:1,name:'空の計画'}]);
 expect(compactEmptyPlans(stationOnly).plannedStations).toEqual(stationOnly.plannedStations);
 expect(base.plans).toHaveLength(2);
});
