import {expect,test} from 'vitest';
import {emptyConstructionPlans,removeConstructionPlan} from './constructionPlans';
test('station-only and redo-only plans survive restoration',()=>{
 const base={...emptyConstructionPlans(),plans:[{id:1,name:''},{id:2,name:''}],activePlanId:2,redoPoints:[{latitude:35,longitude:139}],plannedStations:[{id:'planned:1',planId:1,alignmentId:'3',name:'駅',point:{latitude:35,longitude:139},lengthM:100}]};
 expect(base.plans).toHaveLength(2);
 const result=removeConstructionPlan(base,2);expect(result.plannedStations).toEqual(base.plannedStations);expect(result.activePlanId).toBe(1);expect(result.redoPoints).toEqual([]);
});
