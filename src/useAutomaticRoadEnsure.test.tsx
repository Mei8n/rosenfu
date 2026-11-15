// @vitest-environment jsdom
import {act,cleanup,renderHook} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
const mocks=vi.hoisted(()=>({load:vi.fn()}));
vi.mock('./roadProvider',()=>({createRoadProvider:()=>({name:'fixture',load:mocks.load})}));
import {useAutomaticRoad} from './useAutomaticRoad';
import type {RoadSnapshot} from './automaticRoadData';
const base={west:139.70,east:139.72,south:35.62,north:35.64};
const snapshot={roads:[],bundle:{}} as unknown as RoadSnapshot;
afterEach(()=>{cleanup();vi.useRealTimers();vi.clearAllMocks();});
test('early ensure and debounced display share exactly one pending provider request',async()=>{
 vi.useFakeTimers();let finish!:(value:RoadSnapshot)=>void;mocks.load.mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
 const {result}=renderHook(()=>useAutomaticRoad(base,true,1));
 const promise=result.current.ensureCurrent();await act(()=>vi.advanceTimersByTimeAsync(500));expect(mocks.load).toHaveBeenCalledTimes(1);
 await act(async()=>finish(snapshot));expect(await promise).toEqual({status:'ready',snapshot});
});
test('deadline includes queued time and later completion cannot revive the expired build',async()=>{
 vi.useFakeTimers();let finish!:(value:RoadSnapshot)=>void;mocks.load.mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
 const {result}=renderHook(()=>useAutomaticRoad(base,true,1));const promise=result.current.ensureCurrent();
 await act(()=>vi.advanceTimersByTimeAsync(25000));expect(await promise).toMatchObject({status:'unavailable',reason:expect.stringContaining('25秒')});
 await act(async()=>finish(snapshot));expect(result.current.status).toBe('error');
});
test.each(['world','bounds'])('%s change rejects old result without initiating construction',async changed=>{
 vi.useFakeTimers();let finish!:(value:RoadSnapshot)=>void;mocks.load.mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
 const {result,rerender}=renderHook(({world,bounds})=>useAutomaticRoad(bounds,true,world),{initialProps:{world:1,bounds:base}});
 const promise=result.current.ensureCurrent();rerender(changed==='world'?{world:2,bounds:base}:{world:1,bounds:{...base,west:139.705}});await act(async()=>finish(snapshot));expect(await promise).toEqual({status:'stale'});
});
test('cancel returns promptly while preserving a shared download for other consumers',async()=>{
 vi.useFakeTimers();let finish!:(value:RoadSnapshot)=>void;mocks.load.mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
 const {result}=renderHook(()=>useAutomaticRoad(base,true,1));const controller=new AbortController();const promise=result.current.ensureCurrent(controller.signal);
 await act(async()=>controller.abort());expect(await promise).toEqual({status:'stale'});expect(mocks.load).toHaveBeenCalledTimes(1);await act(async()=>finish(snapshot));
});
