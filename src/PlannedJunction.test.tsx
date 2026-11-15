import {LEVEL_LABELS} from './sectionInfrastructure';
// @vitest-environment jsdom
import React from "react";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { App } from "./App";
import { project, DEFAULT_CAMERA, unproject } from "./map";
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
  vi.mocked(invoke).mockImplementation(async command => {
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
test('midline click creates planned exact source without mutating built track, then sends atomic source snapshot',async()=>{
 const source={id:'3',owner:'2',name:'本線',track_type:'normal',points:[unproject(100,300),unproject(900,300)],anchor_indices:[0,1]};
 const map=await start({...empty,next_id:'4',alignments:[source]});
 fireEvent.click(map,{clientX:200,clientY:151});fireEvent.pointerMove(map,{clientX:300,clientY:170});fireEvent.click(map,{clientX:300,clientY:170});fireEvent.click(map,{clientX:300,clientY:170});
 expect(map.querySelectorAll('[data-alignment-id]')).toHaveLength(1);
 expect(vi.mocked(invoke).mock.calls.map(([c])=>c)).toEqual(['engine_info','world_snapshot','list_save_slots','autosave_status']);
 const original=vi.mocked(invoke).getMockImplementation()!;vi.mocked(invoke).mockImplementation((c,a)=>c==='build_plan'?Promise.reject(Error('junction inspected')):original(c,a));
 fireEvent.click(trackPanel().getByRole('button',{name:'計画を着工'}));await screen.findByText(/junction inspected/);
 const args=vi.mocked(invoke).mock.calls.find(([c])=>c==='build_plan')![1] as {alignments:{junctionSource:{fraction:number;expectedAlignment:unknown;reverse:boolean};points:unknown[]}[]};
 expect(args.alignments[0].junctionSource.fraction).toBeCloseTo(.375);expect(args.alignments[0].junctionSource.expectedAlignment).toEqual(source);expect(args.alignments[0].junctionSource.reverse).toBe(false);
 expect(map.querySelectorAll('[data-alignment-id]')).toHaveLength(1);expect(map.querySelectorAll('.draft-anchor').length).toBeGreaterThan(0);
});
test('junction source survives Undo Redo and save reload without changing original infrastructure',async()=>{
 const source={id:'3',owner:'2',name:'本線',track_type:'normal',points:[unproject(100,300),unproject(900,300)],anchor_indices:[0,1]};
 let map=await start({...empty,next_id:'4',alignments:[source]});
 fireEvent.click(map,{clientX:200,clientY:150});fireEvent.click(map,{clientX:100,clientY:170});fireEvent.click(map,{clientX:100,clientY:170});
 await act(async()=>{fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});});await act(async()=>{fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});});
 const original=vi.mocked(invoke).getMockImplementation()!;vi.mocked(invoke).mockImplementation((c,a)=>c==='save_world'?Promise.resolve({saved:true}):original(c,a));
 fireEvent.click(screen.getByRole('button',{name:'保存'}));await screen.findByText('ワールドと未着工計画を保存しました。');
 const saved=(vi.mocked(invoke).mock.calls.find(([c])=>c==='save_world')![1] as {constructionPlans:import('./constructionPlans').ConstructionPlans}).constructionPlans;
 expect(saved.draftJunctionSource?.reverse).toBe(true);expect(saved.draftJunctionSource?.expectedAlignment).toEqual(source);
 cleanup();map=await start({...empty,next_id:'4',alignments:[source],construction_plans:saved});expect(screen.queryByText('未保存の変更あり')).toBeNull();expect(map.querySelectorAll('[data-alignment-id]')).toHaveLength(1);
});
test('successful branch construction uses guarded world Undo and exact build Redo',async()=>{
 const source={id:'3',owner:'2',name:'本線',track_type:'normal',points:[unproject(100,300),unproject(900,300)],anchor_indices:[0,1]};
 const before={...empty,next_id:'4',alignments:[source]},after={...before,next_id:'6',alignments:[source,{...source,id:'4'},{...source,id:'5'}]};
 const map=await start(before);fireEvent.click(map,{clientX:200,clientY:150});fireEvent.click(map,{clientX:300,clientY:170});fireEvent.click(map,{clientX:300,clientY:170});
 vi.mocked(invoke).mockImplementation(async c=>c==='build_plan'?after:c==='undo_alignment_deletion'?before:{});
 await act(async()=>{fireEvent.click(trackPanel().getByRole('button',{name:'計画を着工'}));});expect(map.querySelectorAll('[data-alignment-id]')).toHaveLength(3);
 await act(async()=>{fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});});expect(map.querySelectorAll('[data-alignment-id]')).toHaveLength(1);expect(invoke).toHaveBeenCalledWith('undo_alignment_deletion');
 await act(async()=>{fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});});expect(map.querySelectorAll('[data-alignment-id]')).toHaveLength(3);
 const calls=vi.mocked(invoke).mock.calls.filter(([c])=>c==='build_plan');expect(calls).toHaveLength(2);expect(calls[1][1]).toEqual(calls[0][1]);
});
