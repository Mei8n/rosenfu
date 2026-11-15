// Existing editor tests isolate the new environmental service. Its real IPC contract,
// async states and core behaviour are covered by dedicated environment tests.
import {vi} from 'vitest';
vi.mock('./useAutomaticEnvironment',()=>({useAutomaticEnvironment:()=>({road:{status:'ready',message:'道路fixture'},water:{status:'ready',message:'水域fixture'},data:{revision:'test',road_coverage:[],roads:[],water_coverage:[],water:[]},retry:()=>{}})}));
vi.mock('./useEnvironmentPreview',()=>({useEnvironmentPreview:()=>({preview:{buildable:true,crossings:[],assessments:[]}})}));
vi.mock('./submitEnvironmentConstruction',async()=>{const {invoke}=await import('@tauri-apps/api/core');return {submitEnvironmentConstruction:(requests:Record<string,unknown>[],stations:unknown[])=>{
 const first=requests[0];
 const batch=stations.length>0||requests.length>1||requests.some(r=>r.extension||r.reference);
 return invoke(batch?'build_plan':(first.stations as unknown[]).length?'build_alignment_with_stations':'build_alignment',batch?{alignments:requests,stations}:(first.stations as unknown[]).length?first:{name:first.name,points:first.points,anchorIndices:first.anchorIndices,trackType:first.trackType,designSpeedKmh:first.designSpeedKmh,infrastructure:first.infrastructure});
}};});
