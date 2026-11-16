// @vitest-environment jsdom
import {act,cleanup,renderHook,waitFor} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {useConstructionPreview} from './useConstructionPreview';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
test('preview uses complete request and ignores delayed old-world results',async()=>{
 let oldResolve:(value:unknown)=>void=()=>{};const old={},next={},request={alignments:[{anchorLevels:['ground','elevated','ground']}],stations:[]};
 vi.mocked(invoke).mockImplementationOnce(()=>new Promise(resolve=>oldResolve=resolve)).mockResolvedValueOnce({ok:true});
 const {result,rerender}=renderHook(({world})=>useConstructionPreview(world,request),{initialProps:{world:old}});
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('preview_construction_plan',{...request,roads:[]}));
 rerender({world:next});expect(result.current).toBeNull();
 await act(async()=>oldResolve({stale:true}));
 await waitFor(()=>expect(result.current?.value).toEqual({ok:true}));
});
test('changed request immediately invalidates prior refusal',async()=>{
 vi.mocked(invoke).mockRejectedValue('距離不足');const world={};
 const {result,rerender}=renderHook(({request})=>useConstructionPreview(world,request),{initialProps:{request:{alignments:[{name:'A'}],stations:[]}}});
 await waitFor(()=>expect(result.current?.error).toContain('距離不足'));
 rerender({request:{alignments:[{name:'B'}],stations:[]}});expect(result.current).toBeNull();
});

test('newly installed water input invalidates the previous preview',async()=>{
 vi.mocked(invoke).mockRejectedValueOnce('細密未取得').mockRejectedValueOnce('水域上に地上不可');const world={},request={alignments:[{}],stations:[]};
 const {result,rerender}=renderHook(({revision})=>useConstructionPreview(world,request,revision),{initialProps:{revision:0}});
 await waitFor(()=>expect(result.current?.error).toBe('細密未取得'));
 rerender({revision:1});expect(result.current).toBeNull();await waitFor(()=>expect(result.current?.error).toBe('水域上に地上不可'));
});

test('slow native preview never queues every intermediate cursor position',async()=>{
 vi.useFakeTimers();
 let finish:(value:unknown)=>void=()=>{};
 vi.mocked(invoke).mockImplementationOnce(()=>new Promise(resolve=>finish=resolve)).mockResolvedValue({ok:true});
 const world={},make=(name:string)=>({alignments:[{name}],stations:[]});
 const {rerender,result,unmount}=renderHook(({request})=>useConstructionPreview(world,request),{initialProps:{request:make('A')}});
 try {
  await act(async()=>{vi.advanceTimersByTime(150);});
  for(const name of ['B','C','D']){rerender({request:make(name)});await act(async()=>{vi.advanceTimersByTime(150);});}
  expect(invoke).toHaveBeenCalledTimes(1);
  await act(async()=>finish({old:true}));
  expect(invoke).toHaveBeenCalledTimes(2);
  expect(invoke).toHaveBeenLastCalledWith('preview_construction_plan',{...make('D'),roads:[]});
  expect(result.current?.value).toEqual({ok:true});
 }finally{unmount();vi.useRealTimers();}
});
test('unmount drops queued preview without starting another native call',async()=>{
 vi.useFakeTimers();let finish:(value:unknown)=>void=()=>{};
 vi.mocked(invoke).mockImplementation(()=>new Promise(resolve=>finish=resolve));
 const world={},make=(name:string)=>({alignments:[{name}],stations:[]});
 const {rerender,unmount}=renderHook(({request})=>useConstructionPreview(world,request),{initialProps:{request:make('A')}});
 try{
  await act(async()=>{vi.advanceTimersByTime(150);});
  rerender({request:make('B')});await act(async()=>{vi.advanceTimersByTime(150);});
  unmount();await act(async()=>finish(null));expect(invoke).toHaveBeenCalledTimes(1);
 }finally{vi.useRealTimers();}
});
