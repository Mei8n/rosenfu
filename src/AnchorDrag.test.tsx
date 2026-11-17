import {settledPointerMove} from './testMapClick';
import {settledMapClick} from './testMapClick';
// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();vi.unstubAllGlobals();localStorage.clear();});
const empty={next_id:'3',players:[{id:'1',name:'Player'}],companies:[{id:'2',owner:'1',name:'Railway'}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
async function setup(){
 vi.stubGlobal('PointerEvent',MouseEvent);vi.mocked(isTauri).mockReturnValue(true);vi.mocked(invoke).mockImplementation(async command=>command==='preview_construction_plan'?{validationError:null}:command==='engine_info'?{name:'Rosenfu',version:'drag-test'}:command==='world_snapshot'?empty:command==='save_world'?{saved:true}:command==='list_save_slots'?{active_slot_id:'test-slot',slots:[{id:'test-slot',name:'試験',loadable:true}]}:command==='autosave_status'?{available:false}:{});
 render(<App/>);fireEvent.click(screen.getByRole('button',{name:'自作サンプル（オフライン）'}));await screen.findByText(/drag-test/);
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith("autosave_status",{slotId:"test-slot"}));
 await act(async()=>{});
 const map=screen.getByRole('img',{name:'駅配置用サンプル地図'});map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 for(const x of [150,250,350])await settledMapClick(map,{clientX:x,clientY:150});
 return map;
}
function startDrag(map:HTMLElement){const anchor=map.querySelector('[data-anchor-index="1"]')!;fireEvent.pointerDown(anchor,{clientX:250,clientY:150,button:0});fireEvent.pointerMove(map,{clientX:250,clientY:160});}
const shape=(map:HTMLElement)=>map.querySelector('.draft-path:not(.parked-plan)')!.getAttribute('d');
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
test('point addition and anchor movement undo in chronological order',async()=>{
 const map=await setup(),before=shape(map);startDrag(map);fireEvent.pointerUp(map,{clientX:250,clientY:160});const moved=shape(map);
 // Real clicks reset the previous drag click suppression.
 fireEvent.pointerDown(map,{clientX:400,clientY:150,button:0});fireEvent.pointerUp(map,{clientX:400,clientY:150});await settledMapClick(map,{clientX:400,clientY:150});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(4);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(shape(map)).toBe(moved);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(shape(map)).toBe(before);
});
test('clicking last anchor without dragging still finishes the line',async()=>{
 const map=await setup(),anchor=map.querySelector('[data-anchor-index="2"]')!,before=shape(map);
 fireEvent.pointerDown(anchor,{clientX:350,clientY:150,button:0});fireEvent.pointerUp(map,{clientX:350,clientY:150});await settledMapClick(map,{clientX:350,clientY:150});
 expect(shape(map)).toBe(before);expect(map.querySelectorAll('.draft-anchor')).toHaveLength(3);
 fireEvent.pointerDown(map,{clientX:150,clientY:210,button:0});fireEvent.pointerUp(map,{clientX:150,clientY:210});await settledMapClick(map,{clientX:150,clientY:210});
 expect(map.querySelectorAll('.parked-plan')).toHaveLength(1);expect(map.querySelectorAll('.draft-anchor')).toHaveLength(1);
});
test('moving a plan preserves its station but refuses construction until geometry or station is corrected',async()=>{
 const map=await setup();fireEvent.click(screen.getByRole('tab',{name:'駅設置'}));await settledMapClick(map,{clientX:250,clientY:150});expect(map.querySelectorAll('.planned-station')).toHaveLength(1);
 fireEvent.click(screen.getByRole('tab',{name:'線路'}));startDrag(map);fireEvent.pointerUp(map,{clientX:250,clientY:160});
 expect(map.querySelectorAll('.planned-station')).toHaveLength(1);
 fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));await screen.findByText(/計画駅が線路に収まりません/);
 expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='build_alignment_with_stations')).toBe(false);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelectorAll('.planned-station')).toHaveLength(1);
});
test('parking and reselecting edited geometry never regenerates the old curve',async()=>{
 const map=await setup();startDrag(map);fireEvent.pointerUp(map,{clientX:250,clientY:160});const after=shape(map);
 fireEvent.click(screen.getByRole('button',{name:'別の線路計画を追加'}));expect(map.querySelector('.parked-plan')!.getAttribute('d')).toBe(after);
 fireEvent.click(screen.getByRole('button',{name:/計画 1 を編集/}));expect(shape(map)).toBe(after);
});

test('drag previews adjacent segment speed labels instead of an instruction popup',async()=>{
 const map=await setup();startDrag(map);
 expect(map.querySelectorAll('.drag-segment-speed')).toHaveLength(2);
 expect(Array.from(map.querySelectorAll('.drag-segment-speed')).every(label=>label.textContent?.includes('km/h'))).toBe(true);
 expect(screen.queryByText('アンカーを移動')).toBeNull();
 fireEvent.pointerCancel(map);
 expect(map.querySelector('.drag-segment-speed')).toBeNull();
});
async function savedPlan(){
 fireEvent.click(screen.getByRole('button',{name:'保存'}));
 await screen.findByText(/ワールドと未着工計画を保存しました/);
 return (vi.mocked(invoke).mock.calls.filter(([command])=>command==='save_world').at(-1)![1] as any).constructionPlans;
}
const undo=()=>fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
const redo=()=>fireEvent.keyDown(document.body,{key:'z',ctrlKey:true,shiftKey:true});
test('finished path undo/redo retains completion at three, two and zero points',async()=>{
 const map=await setup();await settledMapClick(map,{clientX:350,clientY:150});
 for(const length of [2,0]){
  undo();await settledPointerMove(map,{clientX:420,clientY:180});
  expect(map.querySelectorAll('.draft-anchor')).toHaveLength(length);
  const saved=await savedPlan();expect(saved.draftFinished).toBe(length>0);expect(saved.draftPoints).toHaveLength(length);expect(map.querySelector('.candidate-draft')).toBeNull();
 }
 for(const length of [2,3]){
  redo();await settledPointerMove(map,{clientX:420,clientY:180});
  const saved=await savedPlan();expect(saved.draftFinished).toBe(length>0);expect(saved.draftPoints).toHaveLength(length);expect(map.querySelector('.candidate-draft')).toBeNull();
 }
});
test('loaded finished plans retain completion without synthesizing session history',async()=>{
 const map=await setup();await settledMapClick(map,{clientX:350,clientY:150});const saved=await savedPlan();
 vi.mocked(invoke).mockResolvedValueOnce({...empty,construction_plans:saved});fireEvent.click(screen.getByRole('button',{name:'読込'}));await screen.findByText(/保存したワールドを読み込みました/);
 undo();expect((await savedPlan()).draftFinished).toBe(true);expect(map.querySelectorAll('.draft-anchor')).toHaveLength(3);
 redo();expect((await savedPlan()).draftFinished).toBe(true);expect(map.querySelectorAll('.draft-anchor')).toHaveLength(3);
});
test('finished drag undo and redo preserve completed state and other paths',async()=>{
 const map=await setup();fireEvent.click(screen.getByRole('button',{name:'別の線路計画を追加'}));const parked=map.querySelector('.parked-plan')!.getAttribute('d');
 for(const x of [150,250,350])await settledMapClick(map,{clientX:x,clientY:210});
 await settledMapClick(map,{clientX:350,clientY:210});const before=shape(map);
 const anchor=map.querySelector('[data-anchor-index="1"][data-anchor-path="2"]')!;
 fireEvent.pointerDown(anchor,{clientX:250,clientY:210,button:0});fireEvent.pointerMove(map,{clientX:250,clientY:220});fireEvent.pointerUp(map,{clientX:250,clientY:220});
 const after=shape(map);expect(after).not.toBe(before);
 undo();expect(shape(map)).toBe(before);expect((await savedPlan()).draftFinished).toBe(true);
 redo();expect(shape(map)).toBe(after);expect((await savedPlan()).draftFinished).toBe(true);expect(map.querySelector('.parked-plan')!.getAttribute('d')).toBe(parked);
});
