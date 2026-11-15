import {it,expect,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {submitEnvironmentConstruction} from './submitEnvironmentConstruction';
import {assessmentBuildable,assessmentMessage} from './environmentData';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
it('submits the exact preview snapshot to the checked commit command',async()=>{const data={revision:'fixture',roads:[],road_coverage:[],water:[],water_coverage:[]},paths=[{points:[1,2]}];await submitEnvironmentConstruction(paths,[],data);expect(invoke).toHaveBeenCalledWith('build_plan_with_environment',{alignments:paths,stations:[],environment:data});});
it('never sends a build without data or disguises unknown as zero crossings',async()=>{vi.mocked(invoke).mockClear();await expect(submitEnvironmentConstruction([{}],[],null)).rejects.toThrow('未取得');expect(invoke).not.toHaveBeenCalled();const a={crossings:[],roads_ready:false,water_ready:false,unknown_road_height:false,ground_water:false,longest_water_m:0,water_limit_exceeded:false};expect(assessmentBuildable(a)).toBe(false);expect(assessmentMessage(a)).toContain('未取得');});
