import {LEVEL_LABELS} from './sectionInfrastructure';
// @vitest-environment jsdom
import React from "react";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { App } from "./App";
import { project, DEFAULT_CAMERA } from "./map";
import { alignmentLength, pointOnAlignment } from "./alignment";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
HTMLDialogElement.prototype.showModal=function(){this.setAttribute("open","");};
HTMLDialogElement.prototype.close=function(){this.removeAttribute("open");};
afterEach(() => { localStorage.removeItem("rosenfu.display-settings"); cleanup(); vi.resetAllMocks(); });
const alignment = { id:"3",owner:"2",name:"試験線",points:[{latitude:35.67504,longitude:139.72},{latitude:35.67504,longitude:139.78}] };
const trackPanel = () => within(document.querySelector<HTMLElement>(".construction-content")!);
const empty = {next_id:"3",players:[{id:"1",name:"Player"}],companies:[{id:"2",owner:"1",name:"Railway"}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
const built = {...empty,next_id:"4",alignments:[alignment]};
const placed = {...built,next_id:"5",stations:[{id:"4",owner:"2",name:"新駅",latitude:35.67504,longitude:139.75}],station_placements:[{station_id:"4",alignment_id:"3",position:.5,length_m:200,width_m:12}]};
async function start(snapshot: unknown = empty) {
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke).mockImplementation(async command => {if(command==="preview_construction_plan")return {validationError:null};
    if(command==="engine_info") return {name:"Rosenfu",version:"construction-test"};
    if(command==="world_snapshot") return snapshot;
    if(command==="list_save_slots") return {active_slot_id:"test-slot",slots:[{id:"test-slot",name:"試験",loadable:true}]};
    if(command==="autosave_status") return {available:false};
    throw Error("unexpected "+command);
  });
  render(<App/>); fireEvent.click(screen.getByRole("button",{name:"自作サンプル（オフライン）"})); await screen.findByText(/construction-test/);
  // Version renders before slot discovery. Resolve startup reads before tests
  // issue load actions or install one-shot mutation responses.
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("autosave_status",{slotId:"test-slot"}));
  await act(async()=>{});
  const map=screen.getByRole("img",{name:"駅配置用サンプル地図"});
  map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
  return map;
}
test('confirmed invalid hover cannot add the matching anchor',async()=>{
 if(!window.PointerEvent)window.PointerEvent=MouseEvent as typeof PointerEvent;
 const map=await start();const original=vi.mocked(invoke).getMockImplementation()!;
 vi.mocked(invoke).mockImplementation((command,args)=>command==='preview_construction_plan'?Promise.resolve({validationError:'試験の敷設不可'}):original(command,args));
 fireEvent.click(map,{clientX:100,clientY:100});
 fireEvent.pointerMove(map,{clientX:300,clientY:100,pointerId:1,pointerType:'mouse'});
 await screen.findByText('敷設不可：試験の敷設不可');
 fireEvent.click(map,{clientX:300,clientY:100});
 expect(map.querySelectorAll('[data-anchor-index]').length).toBe(1);
});
test('preview transport failure requests retry without misreporting unbuildable',async()=>{
 if(!window.PointerEvent)window.PointerEvent=MouseEvent as typeof PointerEvent;
 const map=await start();const original=vi.mocked(invoke).getMockImplementation()!;
 vi.mocked(invoke).mockImplementation((command,args)=>command==='preview_construction_plan'?Promise.reject(Error('offline')):original(command,args));
 fireEvent.click(map,{clientX:100,clientY:100});
 fireEvent.pointerMove(map,{clientX:300,clientY:100,pointerId:1,pointerType:'mouse'});
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('preview_construction_plan',expect.anything()));
 await act(async()=>{});
 fireEvent.click(map,{clientX:300,clientY:100});
 await screen.findByText(/敷設判定を取得できません。もう一度クリックして再試行してください。/);
 expect(map.querySelectorAll('[data-anchor-index]').length).toBe(1);
 expect(screen.queryByText(/敷設不可：.*offline/)).toBeNull();
});
test('a quick click waits for its own verdict and rejects invalid placement',async()=>{
 const map=await start();const original=vi.mocked(invoke).getMockImplementation()!;
 let complete!:(value:unknown)=>void;
 vi.mocked(invoke).mockImplementation((command,args)=>command==='preview_construction_plan'?('roads' in (args as object)?new Promise(resolve=>{complete=resolve;}):Promise.resolve({validationError:null})):original(command,args));
 fireEvent.click(map,{clientX:100,clientY:100});fireEvent.click(map,{clientX:300,clientY:100});
 expect(map.querySelectorAll('[data-anchor-index]').length).toBe(1);
 await act(async()=>complete({validationError:'急勾配'}));
 expect(map.querySelectorAll('[data-anchor-index]').length).toBe(1);
 expect(await screen.findByText(/処理できませんでした: 急勾配/)).toBeTruthy();
});
test('successful delayed click cannot insert after switching tools',async()=>{
 const map=await start();const original=vi.mocked(invoke).getMockImplementation()!;
 let complete!:(value:unknown)=>void;
 vi.mocked(invoke).mockImplementation((command,args)=>command==='preview_construction_plan'?('roads' in (args as object)?new Promise(resolve=>{complete=resolve;}):Promise.resolve({validationError:null})):original(command,args));
 fireEvent.click(map,{clientX:100,clientY:100});fireEvent.click(map,{clientX:300,clientY:100});
 fireEvent.click(screen.getByRole('button',{name:'選択'}));
 await act(async()=>complete({validationError:null}));
 expect(map.querySelectorAll('[data-anchor-index]').length).toBe(1);
});
test('delayed valid double click adds the anchor and ends editing',async()=>{
 const map=await start();const original=vi.mocked(invoke).getMockImplementation()!;
 let complete!:(value:unknown)=>void;
 vi.mocked(invoke).mockImplementation((command,args)=>command==='preview_construction_plan'?('roads' in (args as object)?new Promise(resolve=>{complete=resolve;}):Promise.resolve({validationError:null})):original(command,args));
 fireEvent.click(map,{clientX:100,clientY:100});fireEvent.click(map,{clientX:300,clientY:100,detail:1});fireEvent.click(map,{clientX:300,clientY:100,detail:2});
 expect(map.querySelectorAll('[data-anchor-index]').length).toBe(1);
 await screen.findByText('この位置に敷設できるか確認中…');
 await act(async()=>complete({validationError:null}));
 expect(map.querySelectorAll('[data-anchor-index]').length).toBe(2);
 fireEvent.pointerMove(map,{clientX:400,clientY:100,pointerId:1,pointerType:'mouse'});
 expect(map.querySelector('.candidate-draft')).toBeNull();
});
test('construction cannot race a pending third-anchor verdict',async()=>{
 const map=await start();
 await act(async()=>{fireEvent.click(map,{clientX:100,clientY:100});});
 await act(async()=>{fireEvent.click(map,{clientX:250,clientY:100});});
 expect(map.querySelectorAll('[data-anchor-index]').length).toBe(2);
 const original=vi.mocked(invoke).getMockImplementation()!;
 let complete!:(value:unknown)=>void;
 vi.mocked(invoke).mockImplementation((command,args)=>command==='preview_construction_plan'?new Promise(resolve=>{complete=resolve;}):original(command,args));
 fireEvent.click(map,{clientX:400,clientY:100});
 const build=screen.getByRole('button',{name:'計画を着工'});
 expect((build as HTMLButtonElement).disabled).toBe(true);
 fireEvent.click(build);
 expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='build_alignment'||command==='build_plan')).toBe(false);
 await act(async()=>complete({validationError:null}));
 expect(map.querySelectorAll('[data-anchor-index]').length).toBe(3);
 expect((build as HTMLButtonElement).disabled).toBe(false);
 fireEvent.click(build);
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('build_alignment',expect.objectContaining({anchorIndices:expect.any(Array)})));
 const builtRequest=vi.mocked(invoke).mock.calls.find(([command])=>command==='build_alignment')![1] as {anchorIndices:number[]};
 expect(builtRequest.anchorIndices).toHaveLength(3);
});
