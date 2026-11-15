// @vitest-environment jsdom
import {act,renderHook,cleanup} from '@testing-library/react';
import {afterEach,expect,it,vi} from 'vitest';
const mock=vi.hoisted(()=>({roads:vi.fn(),water:vi.fn()}));
vi.mock('./automaticRoadData',async()=>{const actual=await vi.importActual<typeof import('./automaticRoadData')>('./automaticRoadData');return {...actual,createRoadRepository:()=>({load:mock.roads})};});
vi.mock('./automaticWaterData',()=>({createWaterRepository:()=>({load:mock.water})}));
import {useAutomaticEnvironment} from './useAutomaticEnvironment';
import type {RoadSnapshot} from './automaticRoadData';
afterEach(()=>{cleanup();vi.useRealTimers();});
it('rejects late pan and world responses, while retaining explicitly saved offline coverage',async()=>{
 vi.useFakeTimers();const pending:((v:RoadSnapshot)=>void)[]=[];
 mock.roads.mockImplementation(()=>new Promise<RoadSnapshot>(resolve=>pending.push(resolve)));mock.water.mockResolvedValue({coverage:[],water:[]});
 const first={west:139,south:35,east:139.01,north:35.01},second={...first,west:139.02,east:139.03};
 const saved={revision:'saved',roads:[],road_coverage:[first],water:[],water_coverage:[first]};
 const snapshot=(bounds:typeof first,id:string)=>({bounds,fetchedAt:1,roads:[],bundle:{graphSha256:id,graph:{nodes:[],edges:[]}}} as unknown as RoadSnapshot);
 const {result,rerender}=renderHook(({bounds,epoch})=>useAutomaticEnvironment(bounds,true,epoch,saved),{initialProps:{bounds:first,epoch:0}});
 await act(async()=>{await vi.advanceTimersByTimeAsync(500);});
 rerender({bounds:second,epoch:0});await act(async()=>{await vi.advanceTimersByTimeAsync(500);});
 await act(async()=>{pending[0](snapshot(first,'old'));});expect(result.current.road.snapshot).toBeUndefined();expect(result.current.data?.revision).toBe('saved');
 await act(async()=>{pending[1](snapshot(second,'new'));});expect(result.current.data?.revision).toBe('new');
 rerender({bounds:second,epoch:1});expect(result.current.road.snapshot).toBeUndefined();expect(result.current.data?.revision).toBe('saved');
});
it('fetches a long corridor sequentially and never publishes partial success',async()=>{
 vi.useFakeTimers();mock.roads.mockReset();mock.water.mockReset();
 mock.roads.mockImplementation(async(bounds)=>({bounds,fetchedAt:1,roads:[],bundle:{graphSha256:'corridor'}}));
 mock.water.mockImplementation(async(bounds)=>({coverage:[bounds],water:[]}));
 const bounds={west:139,south:35,east:139.01,north:35.01};
 const lines=[[{longitude:139,latitude:35.001},{longitude:139.3,latitude:35.001}]];
 const {result}=renderHook(()=>useAutomaticEnvironment(bounds,true,0,undefined,lines));
 expect(result.current.data).toBeNull();
 await act(async()=>{await vi.advanceTimersByTimeAsync(500);});
 expect(result.current.data!.road_coverage.length).toBeGreaterThan(10);
 expect(result.current.data!.water_coverage.length).toBe(result.current.data!.road_coverage.length);
 mock.roads.mockClear();
});
it('stops the corridor on water failure and leaves coverage unknown',async()=>{
 vi.useFakeTimers();mock.roads.mockReset();mock.water.mockReset();
 mock.roads.mockImplementation(async(bounds)=>({bounds,fetchedAt:1,roads:[],bundle:{graphSha256:'corridor'}}));
 mock.water.mockRejectedValue(Error('offline'));
 const bounds={west:139,south:35,east:139.01,north:35.01};
 const lines=[[{longitude:139,latitude:35.001},{longitude:139.3,latitude:35.001}]];
 const {result}=renderHook(()=>useAutomaticEnvironment(bounds,true,0,undefined,lines));
 await act(async()=>{await vi.advanceTimersByTimeAsync(500);});
 expect(mock.roads).toHaveBeenCalledTimes(1);expect(result.current.data).toBeNull();expect(result.current.water.status).toBe('error');
});
