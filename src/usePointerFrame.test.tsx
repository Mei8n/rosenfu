// @vitest-environment jsdom
import {act,cleanup,renderHook} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {usePointerFrame} from './usePointerFrame';
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
test('keeps only the latest task of each kind and cancels at input and unmount boundaries',()=>{
 let nextId=0;const frames=new Map<number,FrameRequestCallback>();
 vi.stubGlobal('requestAnimationFrame',(run:FrameRequestCallback)=>{frames.set(++nextId,run);return nextId;});
 vi.stubGlobal('cancelAnimationFrame',(id:number)=>frames.delete(id));
 const {result,unmount}=renderHook(()=>usePointerFrame());
 const calls:number[]=[];
 act(()=>{for(let i=0;i<20;i++)result.current.schedule('move',()=>calls.push(i));result.current.schedule('hover',()=>calls.push(100));});
 expect(frames.size).toBe(1);expect(calls).toEqual([]);
 act(()=>{const run=frames.values().next().value!;frames.clear();run(16);});
 expect(calls).toEqual([19,100]);
 result.current.schedule('move',()=>calls.push(200));result.current.cancel();
 expect(frames.size).toBe(0);
 result.current.schedule('move',()=>calls.push(300));unmount();
 expect(frames.size).toBe(0);expect(calls).toEqual([19,100]);
});
