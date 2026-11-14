// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();vi.unstubAllGlobals();localStorage.clear();});
const empty={next_id:'3',players:[{id:'1',name:'Player'}],companies:[{id:'2',owner:'1',name:'Railway'}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
async function setup(){
 vi.stubGlobal('PointerEvent',MouseEvent);vi.mocked(isTauri).mockReturnValue(true);vi.mocked(invoke).mockImplementation(async command=>command==='engine_info'?{name:'Rosenfu',version:'drag-test'}:command==='world_snapshot'?empty:command==='save_world'?{saved:true}:{});
 render(<App/>);fireEvent.click(screen.getByRole('button',{name:'自作サンプル（オフライン）'}));await screen.findByText(/drag-test/);
 const map=screen.getByRole('img',{name:'駅配置用サンプル地図'});map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 for(const x of [150,250,350])fireEvent.click(map,{clientX:x,clientY:150});
 return map;
}
function startDrag(map:HTMLElement){const anchor=map.querySelector('[data-anchor-index="1"]')!;fireEvent.pointerDown(anchor,{clientX:250,clientY:150,button:0});fireEvent.pointerMove(map,{clientX:250,clientY:160});}
const shape=(map:HTMLElement)=>map.querySelector('.draft-path')!.getAttribute('d');
test('anchor drag previews, commits both sides, undo/redo and save preserves explicit geometry',async()=>{
 const map=await setup(),before=shape(map);startDrag(map);
 expect(map.querySelector('.anchor-drag-preview')).not.toBeNull();expect(shape(map)).toBe(before);expect((screen.getByRole('button',{name:'保存'}) as HTMLButtonElement).disabled).toBe(true);
 fireEvent.pointerUp(map,{clientX:250,clientY:160});const after=shape(map);expect(after).not.toBe(before);expect(map.querySelectorAll('.draft-anchor')).toHaveLength(3);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(shape(map)).toBe(before);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true,shiftKey:true});expect(shape(map)).toBe(after);
 fireEvent.click(screen.getByRole('button',{name:'保存'}));await screen.findByText(/ワールドと未着工計画を保存しました/);
 const payload=vi.mocked(invoke).mock.calls.find(([command])=>command==='save_world')![1] as any;
 expect(payload.constructionPlans.draftEditedGeometry.version).toBe(1);expect(payload.constructionPlans.draftEditedGeometry.line.length).toBeGreaterThan(3);
 vi.mocked(invoke).mockResolvedValueOnce({...empty,construction_plans:payload.constructionPlans});fireEvent.click(screen.getByRole('button',{name:'読込'}));await screen.findByText(/保存したワールドを読み込みました/);expect(shape(map)).toBe(after);
});
test('Escape and pointercancel never commit preview or leave stale click suppression',async()=>{
 const map=await setup(),before=shape(map);startDrag(map);fireEvent.keyDown(document.body,{key:'Escape'});expect(shape(map)).toBe(before);expect(map.querySelector('.anchor-drag-preview')).toBeNull();
 startDrag(map);fireEvent.pointerCancel(map);expect(shape(map)).toBe(before);expect(map.querySelector('.anchor-drag-preview')).toBeNull();
});
