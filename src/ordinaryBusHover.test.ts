import {afterEach,expect,it,vi} from 'vitest';
import {createOrdinaryBusHover} from './ordinaryBusHover';
afterEach(()=>vi.useRealTimers());
const work=(query:string)=>({key:query,query});

it('never runs without a prepared payload and coalesces jittered points to one eligible query',async()=>{
 vi.useFakeTimers();
 const run=vi.fn(async(q:string)=>q),show=vi.fn(),h=createOrdinaryBusHover(run,show);
 h.offer(null);await vi.advanceTimersByTimeAsync(500);expect(run).not.toHaveBeenCalled();
 h.offer(work('A'));h.offer(work('A'));
 await vi.advanceTimersByTimeAsync(40);h.offer(work('B'));
 await vi.advanceTimersByTimeAsync(119);expect(run).not.toHaveBeenCalled();
 await vi.advanceTimersByTimeAsync(1);expect(run.mock.calls).toEqual([['B',expect.any(AbortSignal)]]);
 expect(show).toHaveBeenLastCalledWith('B');
 h.offer(null);h.offer(work('B'));
 expect(show).toHaveBeenLastCalledWith('B');
 expect(run).toHaveBeenCalledTimes(1);
});

it('one active invocation, no stale response, no retry of a failed hover',async()=>{
 vi.useFakeTimers();
 let resolve!:(r:string)=>void;
 const run=vi.fn(async(q:string)=>q==='A'?new Promise<string>(r=>{resolve=r;}):Promise.reject(Error('missing proof')));
 const show=vi.fn(),h=createOrdinaryBusHover(run,show);
 h.offer(work('A'));await vi.advanceTimersByTimeAsync(130);
 h.offer(work('B'));await vi.advanceTimersByTimeAsync(500);expect(run).toHaveBeenCalledTimes(1);
 resolve('stale-A');await vi.advanceTimersByTimeAsync(130);
 expect(show).not.toHaveBeenCalledWith('stale-A');
 expect(run).toHaveBeenCalledTimes(2);
 await vi.advanceTimersByTimeAsync(600);expect(run).toHaveBeenCalledTimes(2);
});

it('cancel clears scope/cache and ignores late in-flight response',async()=>{
 vi.useFakeTimers();
 let resolve!:(r:string)=>void;
 const run=vi.fn((_:string,signal:AbortSignal)=>new Promise<string>(r=>{
  resolve=r;signal.addEventListener('abort',()=>r('late'));
 }));
 const show=vi.fn(),h=createOrdinaryBusHover(run,show);
 h.offer(work('A'));await vi.advanceTimersByTimeAsync(130);
 h.invalidate();await vi.advanceTimersByTimeAsync(0);
 expect(show).not.toHaveBeenCalledWith('late');
 h.offer(work('A'));await vi.advanceTimersByTimeAsync(130);
 expect(run).toHaveBeenCalledTimes(2);resolve('fresh');
 await vi.advanceTimersByTimeAsync(0);expect(show).toHaveBeenLastCalledWith('fresh');
});
