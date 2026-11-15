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
test('App replaces schematic lanes with native shared geometry and preserves hit targets',async()=>{
 const a={id:'3',owner:'2',name:'A',points:[unproject(100,300),unproject(500,300)],infrastructure:[{track_count:2,level:'ground'}]},b={...a,id:'4',points:[unproject(500,300),unproject(900,300)],infrastructure:[{track_count:1,level:'ground'}]};
 const from={endpoint:{alignment_id:'3',endpoint:'end'},lane_index:0},to={endpoint:{alignment_id:'4',endpoint:'start'},lane_index:0};
 const points=[unproject(450,299),unproject(500,300),unproject(550,300)];
 const world={...empty,alignments:[a,b],alignment_connections:[{a:from.endpoint,b:to.endpoint}]};
 vi.mocked(isTauri).mockReturnValue(true);vi.mocked(invoke).mockImplementation(async command=>{
 if(command==='engine_info')return {name:'Rosenfu',version:'turnout-test'};if(command==='world_snapshot')return world;if(command==='list_save_slots')return {active_slot_id:null,slots:[]};if(command==='preview_track_render_geometry')return {lanes:[{alignment_id:'3',section_index:0,lane_index:0,points:a.points},{alignment_id:'4',section_index:0,lane_index:0,points:b.points}],movements:[{movement:{from,to,point:{}},points},{movement:{from:to,to:from,point:{}},points:[...points].reverse()}]};throw Error(command);
 });render(<App/>);await screen.findByText(/turnout-test/);await waitFor(()=>expect(document.querySelectorAll('[data-track-movement]')).toHaveLength(1));expect(document.querySelectorAll('.alignment-hit')).toHaveLength(2);expect(document.querySelectorAll('[data-physical-alignment]')).toHaveLength(2);
 expect(invoke).toHaveBeenCalledWith('preview_track_render_geometry',{world});
 const physical=document.querySelector('.physical-track-geometry')!,overlay=document.querySelector('.route-assignment-overlays')!;
 expect(physical.compareDocumentPosition(overlay)&Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
 expect(overlay.getAttribute('pointer-events')).toBe('none');
});
