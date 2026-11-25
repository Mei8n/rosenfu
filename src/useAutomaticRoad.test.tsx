// @vitest-environment jsdom
import {act,cleanup,renderHook} from '@testing-library/react';
import {afterEach,expect,it,vi} from 'vitest';
const mocks=vi.hoisted(()=>({load:vi.fn()}));
vi.mock('./roadProvider',()=>({createRoadProvider:()=>({name:'fixture',load:mocks.load})}));
import {useAutomaticRoad} from './useAutomaticRoad';
const bounds={west:139.70,east:139.72,south:35.62,north:35.64};
afterEach(()=>{cleanup();vi.useRealTimers();vi.clearAllMocks();});
it('loads roads independently and clears stale data on world change',async()=>{vi.useFakeTimers();mocks.load.mockResolvedValue({bundle:{graph:{}},roads:[]});const {result,rerender}=renderHook(({world})=>useAutomaticRoad(bounds,true,world),{initialProps:{world:1}});await act(()=>vi.advanceTimersByTimeAsync(300));expect(result.current.status).toBe('ready');expect(result.current.message).toContain('網羅性は保証しません');mocks.load.mockImplementation(()=>new Promise(()=>{}));rerender({world:2});expect(result.current.snapshot).toBeUndefined();expect(result.current.status).toBe('loading');});
it('out-of-area failure stays unavailable and retry can recover without water data',async()=>{vi.useFakeTimers();mocks.load.mockRejectedValueOnce(Error('missing cell')).mockResolvedValueOnce({bundle:{graph:{}},roads:[]});const {result}=renderHook(()=>useAutomaticRoad(bounds,true,1));await act(()=>vi.advanceTimersByTimeAsync(300));expect(result.current.status).toBe('error');expect(result.current.snapshot).toBeUndefined();act(()=>result.current.retry());await act(()=>vi.advanceTimersByTimeAsync(300));expect(result.current.status).toBe('ready');});
it('does not load when disabled or frozen',async()=>{vi.useFakeTimers();renderHook(()=>useAutomaticRoad(bounds,false,1));renderHook(()=>useAutomaticRoad(bounds,true,1,true));await act(()=>vi.advanceTimersByTimeAsync(1000));expect(mocks.load).not.toHaveBeenCalled();});

it('aborts obsolete viewport work and cannot publish its delayed completion after panning back',async()=>{
 vi.useFakeTimers();
 const requests:{signal:AbortSignal;finish:(snapshot:unknown)=>void}[]=[];
 mocks.load.mockImplementation((_area,signal:AbortSignal)=>new Promise(resolve=>requests.push({signal,finish:resolve})));
 const moved={...bounds,west:139.75,east:139.77};
 const {result,rerender}=renderHook(({area})=>useAutomaticRoad(area,true,1),{initialProps:{area:bounds}});
 await act(()=>vi.advanceTimersByTimeAsync(300));
 rerender({area:moved});
 expect(requests[0].signal.aborted).toBe(true);
 await act(()=>vi.advanceTimersByTimeAsync(300));
 rerender({area:bounds});
 expect(requests[1].signal.aborted).toBe(true);
 await act(()=>vi.advanceTimersByTimeAsync(300));
 expect(requests).toHaveLength(3);
 await act(async()=>requests[0].finish({roads:[],old:true}));
 expect(result.current.status).toBe('loading');
 const current={roads:[],current:true};
 await act(async()=>requests[2].finish(current));
 expect(result.current.snapshot).toBe(current);
 expect(requests[2].signal.aborted).toBe(false);
});

it('keeps a tile request when a different observer still wants the same viewport',async()=>{
 vi.useFakeTimers();
 let sharedSignal!:AbortSignal;
 let finish!:(snapshot:unknown)=>void;
 mocks.load.mockImplementation((_area,signal:AbortSignal)=>new Promise(resolve=>{sharedSignal=signal;finish=resolve;}));
 const moving=renderHook(()=>useAutomaticRoad(bounds,true,1));
 const staying=renderHook(()=>useAutomaticRoad(bounds,true,2));
 await act(()=>vi.advanceTimersByTimeAsync(300));
 expect(mocks.load).toHaveBeenCalledTimes(1);
 moving.unmount();
 expect(sharedSignal.aborted).toBe(false);
 const snapshot={roads:[]};
 await act(async()=>finish(snapshot));
 expect(staying.result.current.snapshot).toBe(snapshot);
});
