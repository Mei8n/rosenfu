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
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('preview_construction_plan',request));
 rerender({world:next});await waitFor(()=>expect(result.current?.value).toEqual({ok:true}));
 await act(async()=>oldResolve({stale:true}));expect(result.current?.value).toEqual({ok:true});
});
test('changed request immediately invalidates prior refusal',async()=>{
 vi.mocked(invoke).mockRejectedValue('距離不足');const world={};
 const {result,rerender}=renderHook(({request})=>useConstructionPreview(world,request),{initialProps:{request:{alignments:[{name:'A'}],stations:[]}}});
 await waitFor(()=>expect(result.current?.error).toContain('距離不足'));
 rerender({request:{alignments:[{name:'B'}],stations:[]}});expect(result.current).toBeNull();
});
