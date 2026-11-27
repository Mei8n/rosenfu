// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {recordConstructionRender,startConstructionTrace,timeConstructionIPC} from './constructionTiming';
afterEach(()=>{localStorage.clear();vi.unstubAllGlobals();});
function setup(){
 const performance={mark:vi.fn(),measure:vi.fn(),clearMarks:vi.fn(),clearMeasures:vi.fn()};
 vi.stubGlobal('performance',performance);return performance;
}
test('timing is opt-in and never prevents an IPC operation',async()=>{
 const p=setup();expect(startConstructionTrace()).toBeUndefined();
 expect(await timeConstructionIPC('build',async()=>42)).toBe(42);
 expect(p.mark).not.toHaveBeenCalled();expect(p.measure).not.toHaveBeenCalled();
});
test('failed IPC retains the error and records its elapsed span',async()=>{
 const p=setup();localStorage.setItem('rosenfu.construction-timing','1');const trace=startConstructionTrace()!;
 const failure=Error('refused');await expect(timeConstructionIPC('build',async()=>{throw failure;})).rejects.toBe(failure);
 trace.complete();
 expect(p.measure.mock.calls.some(([,options])=>options.detail.phase==='ipc.build')).toBe(true);
});
test('native follow-up IPC can finish after the first frame without being truncated',async()=>{
 const p=setup();localStorage.setItem('rosenfu.construction-timing','1');const trace=startConstructionTrace()!;
 let finish!:(value:number)=>void;const result=timeConstructionIPC('road_assessments',()=>new Promise<number>(resolve=>{finish=resolve;}));
 recordConstructionRender('App','update',12,20,100,120);
 trace.complete();trace.complete();
 expect(p.measure.mock.calls.filter(([,options])=>options.detail.phase==='click-to-first-frame')).toHaveLength(1);
 expect(p.measure.mock.calls.some(([,options])=>options.detail.phase==='ipc.road_assessments')).toBe(false);
 finish(7);expect(await result).toBe(7);
 expect(p.measure.mock.calls.filter(([,options])=>options.detail.phase==='ipc.road_assessments')).toHaveLength(1);
 const render=p.measure.mock.calls.find(([,options])=>options.detail.phase==='react.render')!;
 expect(render[1]).toMatchObject({duration:12,start:100,detail:{commitTime:120}});
});

test('unavailable performance recording cannot turn a successful build into an error',async()=>{
 const p=setup();p.measure.mockImplementation(()=>{throw Error('unsupported measurement');});
 localStorage.setItem('rosenfu.construction-timing','1');const trace=startConstructionTrace()!;
 expect(await timeConstructionIPC('build',async()=>42)).toBe(42);
 expect(()=>trace.complete()).not.toThrow();
});
