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
test('first anchor height stays fixed while destination changes through undo save reload and build request',async()=>{
 let map=await start();await act(async()=>{fireEvent.click(map,{clientX:120,clientY:100});});
 const elevated=screen.getByRole('button',{name:LEVEL_LABELS.elevated});expect(elevated.hasAttribute('disabled')).toBe(false);fireEvent.click(elevated);
 await act(async()=>{fireEvent.click(map,{clientX:300,clientY:100});});await act(async()=>{fireEvent.click(map,{clientX:300,clientY:100});});
 await act(async()=>{fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});});
 await act(async()=>{fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});});
 const original=vi.mocked(invoke).getMockImplementation()!;vi.mocked(invoke).mockImplementation((command,args)=>command==='save_world'?Promise.resolve({saved:true}):original(command,args));
 fireEvent.click(screen.getByRole('button',{name:'保存'}));await screen.findByText('ワールドと未着工計画を保存しました。');
 const saved=(vi.mocked(invoke).mock.calls.find(([c])=>c==='save_world')![1] as {constructionPlans:import('./constructionPlans').ConstructionPlans}).constructionPlans;
 expect(saved.startLevel).toBe('ground');expect(saved.infrastructure?.level).toBe('elevated');
 cleanup();map=await start({...empty,construction_plans:saved});expect(screen.queryByText('未保存の変更あり')).toBeNull();
 vi.mocked(invoke).mockImplementation(async command=>{if(command==="preview_construction_plan")return {validationError:null};if(command==='build_alignment')throw Error('inspect grade');return {};});
 fireEvent.click(trackPanel().getByRole('button',{name:'計画を着工'}));await screen.findByText(/inspect grade/);
 expect(invoke).toHaveBeenCalledWith('build_alignment',expect.objectContaining({startLevel:'ground',infrastructure:{track_count:1,level:'elevated'}}));
});

test('individual anchor levels survive edits Undo Redo save and reload',async()=>{
 let map=await start();
 await act(async()=>{fireEvent.click(map,{clientX:100,clientY:100});});
 fireEvent.click(screen.getByRole('button',{name:LEVEL_LABELS.elevated}));
 await act(async()=>{fireEvent.click(map,{clientX:220,clientY:100});});
 fireEvent.click(screen.getByRole('button',{name:LEVEL_LABELS.ground}));
 await act(async()=>{fireEvent.click(map,{clientX:340,clientY:100});});await act(async()=>{fireEvent.click(map,{clientX:340,clientY:100});});
 fireEvent.click(screen.getByRole('button',{name:'選択'}));fireEvent.click(map.querySelector('[data-anchor-index="1"]')!);
 fireEvent.click(screen.getByRole('button',{name:LEVEL_LABELS.elevated_2}));
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});fireEvent.click(map.querySelector('[data-anchor-index="1"]')!);
 expect(screen.getByRole('button',{name:LEVEL_LABELS.elevated}).getAttribute('aria-pressed')).toBe('true');
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true,shiftKey:true});fireEvent.click(map.querySelector('[data-anchor-index="1"]')!);
 expect(screen.getByRole('button',{name:LEVEL_LABELS.elevated_2}).getAttribute('aria-pressed')).toBe('true');
 const original=vi.mocked(invoke).getMockImplementation()!;vi.mocked(invoke).mockImplementation((command,args)=>command==='save_world'?Promise.resolve({saved:true}):original(command,args));
 fireEvent.click(screen.getByRole('button',{name:'保存'}));await screen.findByText('ワールドと未着工計画を保存しました。');
 const saved=(vi.mocked(invoke).mock.calls.filter(([c])=>c==='save_world').at(-1)![1] as {constructionPlans:import('./constructionPlans').ConstructionPlans}).constructionPlans;
 expect(saved.draftAnchorLevels).toEqual(['ground','elevated_2','ground']);
 cleanup();map=await start({...empty,construction_plans:saved});
 fireEvent.click(screen.getByRole('button',{name:'選択'}));fireEvent.click(map.querySelector('[data-anchor-index="1"]')!);
 expect(screen.getByRole('button',{name:LEVEL_LABELS.elevated_2}).getAttribute('aria-pressed')).toBe('true');
});
test('parked anchor height edits follow chronological Undo history and save levels',async()=>{
 const {emptyConstructionPlans}=await import('./constructionPlans');
 const points=[{latitude:35.67504,longitude:139.73},{latitude:35.67504,longitude:139.76},{latitude:35.67504,longitude:139.79}];
 const plans={...emptyConstructionPlans(),parkedPaths:[{id:2,planId:1,name:'parked',points,line:points,redo:[],trackType:'normal',designSpeedKmh:120,roadFollowing:false,extension:null,anchorLevels:['ground','elevated','ground']}]};
 const map=await start({...empty,construction_plans:plans});fireEvent.click(screen.getByRole('button',{name:'選択'}));fireEvent.click(map.querySelector('[data-anchor-path="2"][data-anchor-index="1"]')!);
 fireEvent.click(screen.getByRole('button',{name:LEVEL_LABELS.elevated_2}));
 fireEvent.click(screen.getByRole('button',{name:LEVEL_LABELS.elevated_3}));
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});fireEvent.click(map.querySelector('[data-anchor-index="1"]')!);expect(screen.getByRole('button',{name:LEVEL_LABELS.elevated_2}).getAttribute('aria-pressed')).toBe('true');
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});fireEvent.click(map.querySelector('[data-anchor-index="1"]')!);expect(screen.getByRole('button',{name:LEVEL_LABELS.elevated}).getAttribute('aria-pressed')).toBe('true');
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true,shiftKey:true});fireEvent.click(map.querySelector('[data-anchor-index="1"]')!);expect(screen.getByRole('button',{name:LEVEL_LABELS.elevated_2}).getAttribute('aria-pressed')).toBe('true');
 const original=vi.mocked(invoke).getMockImplementation()!;vi.mocked(invoke).mockImplementation((command,args)=>command==='save_world'?Promise.resolve({saved:true}):original(command,args));
 fireEvent.click(screen.getByRole('button',{name:'保存'}));await screen.findByText('ワールドと未着工計画を保存しました。');
 expect(invoke).toHaveBeenCalledWith('save_world',expect.objectContaining({constructionPlans:expect.objectContaining({parkedPaths:expect.arrayContaining([expect.objectContaining({id:2,anchorLevels:['ground','elevated_2','ground']})])})}));
});
test('native cursor preview carries per-anchor heights without building',async()=>{
 if(!window.PointerEvent)window.PointerEvent=MouseEvent as typeof PointerEvent;
 const map=await start();const original=vi.mocked(invoke).getMockImplementation()!;
 vi.mocked(invoke).mockImplementation((command,args)=>command==='preview_construction_plan'?Promise.resolve(empty):original(command,args));
 await act(async()=>{fireEvent.click(map,{clientX:100,clientY:100});});fireEvent.click(screen.getByRole('button',{name:LEVEL_LABELS.elevated}));
 fireEvent.pointerMove(map,{clientX:300,clientY:100,pointerId:1,pointerType:'mouse'});
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('preview_construction_plan',expect.objectContaining({alignments:expect.arrayContaining([expect.objectContaining({anchorLevels:['ground','elevated']})])})));
 expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='build_plan'||command==='build_alignment')).toBe(false);
});
