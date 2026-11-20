// Legacy source descriptors remain read-only after the road drawing UI was replaced.
// New click/worker/save/Undo interactions are covered by RoadDrawingApp.test.tsx.
import {expect,test} from 'vitest';
import {emptyConstructionPlans,constructionPlanFingerprint} from './constructionPlans';
import {removePlannedTarget} from './plannedDeletion';
const latitude=35.684,longitude=139.76806640625;
test('whole-path deletion and serialized Undo retain light descriptors; partial mutation is rejected',()=>{const ofmPath={method:'ofm_tile_path_v1'} as any;const point={latitude,longitude};const plans={...emptyConstructionPlans(),parkedPaths:[{id:2,planId:1,name:'road',points:[point,{...point,longitude:longitude+.001}],line:[point,{...point,longitude:longitude+.001}],redo:[],extension:null,trackType:'normal' as const,designSpeedKmh:120,roadFollowing:false,ofmPath}]};const before=JSON.stringify(plans);expect(()=>removePlannedTarget(plans,{kind:'plan-anchor',pathId:2,index:0})).toThrow('参照線路');expect(()=>removePlannedTarget(plans,{kind:'plan-section',pathId:2,sectionIndex:0})).toThrow('参照線路');expect(removePlannedTarget(plans,{kind:'plan-path',pathId:2}).parkedPaths).toHaveLength(0);expect(constructionPlanFingerprint(JSON.parse(before))).toBe(constructionPlanFingerprint(plans));expect(JSON.parse(before).parkedPaths[0].ofmPath).toEqual(ofmPath);});
