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
test('preview transport failure is not a placement refusal',async()=>{
 if(!window.PointerEvent)window.PointerEvent=MouseEvent as typeof PointerEvent;
 const map=await start();const original=vi.mocked(invoke).getMockImplementation()!;
 vi.mocked(invoke).mockImplementation((command,args)=>command==='preview_construction_plan'?Promise.reject(Error('offline')):original(command,args));
 fireEvent.click(map,{clientX:100,clientY:100});
 fireEvent.pointerMove(map,{clientX:300,clientY:100,pointerId:1,pointerType:'mouse'});
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('preview_construction_plan',expect.anything()));
 await act(async()=>{});
 fireEvent.click(map,{clientX:300,clientY:100});
 expect(map.querySelectorAll('[data-anchor-index]').length).toBe(2);
 expect(screen.queryByText(/敷設不可：.*offline/)).toBeNull();
});
