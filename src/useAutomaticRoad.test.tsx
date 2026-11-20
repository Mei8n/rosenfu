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
