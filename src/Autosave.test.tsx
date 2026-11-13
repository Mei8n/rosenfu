// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,renderHook} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {useAutosave} from './useAutosave';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.useRealTimers();vi.resetAllMocks();});
function start(){vi.useFakeTimers();vi.mocked(invoke).mockImplementation(async command=>command==='autosave_status'?{available:false}:{saved:true});const props={ready:true,dirty:true,world:{id:1},epoch:0,busyRef:{current:false},setBusy:vi.fn()};const hook=renderHook(p=>useAutosave(p),{initialProps:props});return {props,...hook};}
test('separate periodic autosave only for changed applied world; respects busy state',async()=>{
 const h=start();await act(async()=>{await vi.advanceTimersByTimeAsync(120000);});expect(invoke).toHaveBeenCalledWith('autosave_world');expect(h.result.current.available).toBe(true);expect(h.props.setBusy.mock.calls).toEqual([[true],[false]]);
 const count=vi.mocked(invoke).mock.calls.length;await act(async()=>{await vi.advanceTimersByTimeAsync(120000);});expect(vi.mocked(invoke).mock.calls.length).toBe(count);
 h.props.busyRef.current=true;h.rerender({...h.props,world:{id:2}});await act(async()=>{await vi.advanceTimersByTimeAsync(120000);});expect(vi.mocked(invoke).mock.calls.length).toBe(count);
 h.props.busyRef.current=false;await act(async()=>{await vi.advanceTimersByTimeAsync(120000);});expect(vi.mocked(invoke).mock.calls.length).toBe(count+1);
});
test('failure stays visible and retries; epoch change does not report stale success',async()=>{
 const h=start();vi.mocked(invoke).mockImplementation(async command=>{if(command==='autosave_status')return {available:false};throw Error('disk full');});await act(async()=>{await vi.advanceTimersByTimeAsync(120000);});expect(h.result.current.message).toContain('disk full');expect(h.props.busyRef.current).toBe(false);
 let finish:()=>void=()=>{};vi.mocked(invoke).mockImplementation(command=>command==='autosave_status'?Promise.resolve({available:false}):new Promise(resolve=>{finish=()=>resolve({saved:true});}));await act(async()=>{await vi.advanceTimersByTimeAsync(120000);});h.rerender({...h.props,epoch:1,world:{id:3}});await act(async()=>{finish();});expect(h.result.current.available).toBe(false);expect(h.props.busyRef.current).toBe(false);expect(h.result.current.message).not.toContain('自動保存しました');
});
test('unmount clears timer and clean world does not autosave',async()=>{const h=start();h.rerender({...h.props,dirty:false});await act(async()=>{await vi.advanceTimersByTimeAsync(120000);});expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='autosave_world')).toBe(false);h.unmount();await act(async()=>{await vi.advanceTimersByTimeAsync(120000);});expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='autosave_world')).toBe(false);});
test('existing slot needs explicit replacement and manual world switch resets permission; successful auto restore continues',async()=>{
 vi.useFakeTimers();vi.mocked(invoke).mockImplementation(async command=>command==='autosave_status'?{available:true}:{saved:true});
 const props={ready:true,dirty:true,world:{id:1},epoch:0,restoredEpoch:-1,busyRef:{current:false},setBusy:vi.fn()};const h=renderHook(p=>useAutosave(p),{initialProps:props});
 await act(async()=>{await vi.advanceTimersByTimeAsync(120000);});expect(h.result.current.permission).toBe('blocked');expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='autosave_world')).toBe(false);
 await act(async()=>{await h.result.current.allowReplacement();});await act(async()=>{await vi.advanceTimersByTimeAsync(120000);});expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==='autosave_world')).toHaveLength(1);
 h.rerender({...props,epoch:1,world:{id:2}});await act(async()=>{await vi.advanceTimersByTimeAsync(120000);});expect(h.result.current.permission).toBe('blocked');expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==='autosave_world')).toHaveLength(1);
 h.rerender({...props,epoch:2,restoredEpoch:2,world:{id:3}});await act(async()=>{await vi.advanceTimersByTimeAsync(120000);});expect(h.result.current.permission).toBe('allowed');expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==='autosave_world')).toHaveLength(2);
});
test('unknown or failed status never writes; retry handles absence and ignores stale checks',async()=>{
 vi.useFakeTimers();let finish:(v:unknown)=>void=()=>{};vi.mocked(invoke).mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
 const props={ready:true,dirty:true,world:{id:1},epoch:0,busyRef:{current:false},setBusy:vi.fn()};const h=renderHook(p=>useAutosave(p),{initialProps:props});
 await act(async()=>{await vi.advanceTimersByTimeAsync(600000);});expect(h.result.current.permission).toBe('checking');expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='autosave_world')).toBe(false);
 vi.mocked(invoke).mockRejectedValueOnce(Error('status denied'));h.rerender({...props,epoch:1});await act(async()=>{finish({available:false});await vi.advanceTimersByTimeAsync(25);});expect(h.result.current.permission).toBe('failed');await act(async()=>{await vi.advanceTimersByTimeAsync(120000);});expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='autosave_world')).toBe(false);
 vi.mocked(invoke).mockImplementation(async c=>c==='autosave_status'?{available:false}:{saved:true});act(()=>h.result.current.retryStatus());await act(async()=>{await vi.advanceTimersByTimeAsync(120000);});expect(invoke).toHaveBeenCalledWith('autosave_world');
});
test('conflicting autosave stays stopped until an explicit recheck; a missing slot can restart',async()=>{
 const h=start();await act(async()=>{});
 vi.mocked(invoke).mockRejectedValueOnce(Error('changed externally'));
 await act(async()=>{await vi.advanceTimersByTimeAsync(120000);});
 expect(h.result.current.permission).toBe('failed');
 const writes=()=>vi.mocked(invoke).mock.calls.filter(([c])=>c==='autosave_world').length;
 const before=writes();await act(async()=>{await vi.advanceTimersByTimeAsync(600000);});expect(writes()).toBe(before);
 await act(async()=>{h.result.current.retryStatus();});expect(h.result.current.permission).toBe('allowed');
 await act(async()=>{await vi.advanceTimersByTimeAsync(120000);});expect(writes()).toBe(before+1);
});
test('replacement authorization failure never starts writing and releases busy state',async()=>{
 const h=start();await act(async()=>{});
 vi.mocked(invoke).mockImplementation(async command=>{if(command==='autosave_status')return {available:true};throw Error('changed after status');});
 await act(async()=>{h.result.current.retryStatus();});expect(h.result.current.permission).toBe('blocked');
 await act(async()=>{await h.result.current.allowReplacement();});expect(h.result.current.permission).toBe('failed');expect(h.props.busyRef.current).toBe(false);
 await act(async()=>{await vi.advanceTimersByTimeAsync(600000);});expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='autosave_world')).toBe(false);
});
test('a delayed old status response cannot revoke a restored epoch grant',async()=>{
 vi.useFakeTimers();let finish:(v:unknown)=>void=()=>{};
 vi.mocked(invoke).mockImplementation(command=>command==='autosave_status'?new Promise(resolve=>{finish=resolve;}):Promise.resolve({saved:true}));
 const props={ready:true,dirty:true,world:{id:1},epoch:0,restoredEpoch:-1,busyRef:{current:false},setBusy:vi.fn()};
 const h=renderHook(p=>useAutosave(p),{initialProps:props});
 expect(h.result.current.permission).toBe('checking');
 h.rerender({...props,epoch:1,restoredEpoch:1,world:{id:2}});
 expect(h.result.current.permission).toBe('allowed');
 await act(async()=>{finish({available:true});});
 expect(h.result.current.permission).toBe('allowed');
 await act(async()=>{await vi.advanceTimersByTimeAsync(120000);});expect(invoke).toHaveBeenCalledWith('autosave_world');
});

test.each(['0','1','5','10'])('fixed two-minute real-time cadence ignores legacy setting %s and survives world edits',async legacy=>{
 localStorage.setItem('rosenfu.autosave-minutes',legacy);
 const h=start();
 const writes=()=>vi.mocked(invoke).mock.calls.filter(([c])=>c==='autosave_world').length;
 await act(async()=>{await vi.advanceTimersByTimeAsync(60000);});
 h.rerender({...h.props,world:{id:2}});
 await act(async()=>{await vi.advanceTimersByTimeAsync(59999);});
 expect(writes()).toBe(0);
 await act(async()=>{await vi.advanceTimersByTimeAsync(1);});
 expect(writes()).toBe(1);
 h.rerender({...h.props,world:{id:3}});
 await act(async()=>{await vi.advanceTimersByTimeAsync(119999);});
 expect(writes()).toBe(1);
 await act(async()=>{await vi.advanceTimersByTimeAsync(1);});
 expect(writes()).toBe(2);
});
