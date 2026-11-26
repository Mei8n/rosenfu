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


test('continuous pointer movement starts preview without a debounce or pause',async()=>{
 vi.useFakeTimers();
 const world={},make=(name:string)=>({alignments:[{name}],stations:[]});
 vi.mocked(invoke).mockResolvedValue({ok:true});
 const {rerender,unmount}=renderHook(({request})=>useConstructionPreview(world,request),{initialProps:{request:make('A')}});
 try{
  expect(invoke).toHaveBeenCalledTimes(1);
  await act(async()=>{});
  for(const name of ['B','C','D']){
   rerender({request:make(name)});
   await act(async()=>{});
   expect(invoke).toHaveBeenLastCalledWith('preview_construction_plan',{...make(name),roads:[]});
  }
  expect(invoke).toHaveBeenCalledTimes(4);
 }finally{unmount();vi.useRealTimers();}
});

test('disabling while native runs drops the latest queued candidate',async()=>{
 let finish:(value:unknown)=>void=()=>{};
 vi.mocked(invoke).mockImplementation(()=>new Promise(resolve=>finish=resolve));
 const world={},make=(name:string)=>({alignments:[{name}],stations:[]});
 const {rerender,result}=renderHook(({request}:{request:ReturnType<typeof make>|null})=>useConstructionPreview(world,request),{initialProps:{request:make('A')} as {request:ReturnType<typeof make>|null}});
 rerender({request:make('B')});rerender({request:null});
 await act(async()=>finish({old:true}));
 expect(invoke).toHaveBeenCalledTimes(1);expect(result.current).toBeNull();
});

test('returning to the exact in-flight shape before it settles needs only one native validation',async()=>{
 let finish!:(value:unknown)=>void;
 vi.mocked(invoke).mockImplementationOnce(()=>new Promise(resolve=>finish=resolve)).mockResolvedValue({repeat:true});
 const world={},make=(last:number)=>({alignments:[{points:[0,last]}],stations:[],roads:[{revision:'road-1'}]});
 const {rerender,result}=renderHook(({request})=>useConstructionPreview(world,request,4),{initialProps:{request:make(1)}});
 await waitFor(()=>expect(finish).toBeTypeOf('function'));
 rerender({request:make(2)});rerender({request:make(1)});
 expect(result.current).toBeNull();
 await act(async()=>finish({exact:true}));
 expect(invoke).toHaveBeenCalledExactlyOnceWith('preview_construction_plan',make(1));
 expect(result.current?.value).toEqual({exact:true});
});

test.each(['disabled','world','water'])('same shape is validated fresh after a %s scope transition',async(kind)=>{
 let finish!:(value:unknown)=>void;
 vi.mocked(invoke).mockImplementationOnce(()=>new Promise(resolve=>finish=resolve)).mockResolvedValue({fresh:true});
 const world={},request={alignments:[{name:'A'}],stations:[]};
 type Props={currentWorld:object;candidate:typeof request|null;revision:number};
 const original:Props={currentWorld:world,candidate:request,revision:4};
 const {rerender,result}=renderHook(({currentWorld,candidate,revision}:Props)=>useConstructionPreview(currentWorld,candidate,revision),{initialProps:original});
 await waitFor(()=>expect(finish).toBeTypeOf('function'));
 rerender(kind==='disabled'?{...original,candidate:null}:kind==='world'?{...original,currentWorld:{}}:{...original,revision:5});
 rerender(original);
 await act(async()=>finish({old:true}));
 expect(invoke).toHaveBeenCalledTimes(2);
 expect(result.current?.value).toEqual({fresh:true});
});

test('a settled result is not redisplayed when the same input is restored after disabling',async()=>{
 let finish!:(value:unknown)=>void;
 vi.mocked(invoke).mockResolvedValueOnce({old:true}).mockImplementationOnce(()=>new Promise(resolve=>finish=resolve));
 const world={},request={alignments:[{name:'A'}],stations:[]};
 const {rerender,result}=renderHook(({enabled})=>useConstructionPreview(world,enabled?request:null),{initialProps:{enabled:true}});
 await waitFor(()=>expect(result.current?.value).toEqual({old:true}));
 rerender({enabled:false});rerender({enabled:true});
 expect(result.current).toBeNull();
 expect(invoke).toHaveBeenCalledTimes(2);
 await act(async()=>finish({fresh:true}));
 expect(result.current?.value).toEqual({fresh:true});
});

test('unrelated rerenders with the same input arrays neither serialize nor restart validation',async()=>{
 const serialize=vi.fn(()=>[]);
 const alignments:unknown[]=[];
 Object.defineProperty(alignments,'toJSON',{value:serialize});
 const stations:unknown[]=[],world={};
 vi.mocked(invoke).mockResolvedValue({ok:true});
 const {rerender,result}=renderHook(()=>useConstructionPreview(world,{alignments,stations}));
 await waitFor(()=>expect(result.current?.value).toEqual({ok:true}));
 serialize.mockClear();vi.mocked(invoke).mockClear();
 for(let i=0;i<10;i++)rerender();
 expect(serialize).not.toHaveBeenCalled();
 expect(invoke).not.toHaveBeenCalled();
});
