import {settledMapClick} from './testMapClick';
// @vitest-environment jsdom
import React from "react";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { App } from "./App";
import { project, DEFAULT_CAMERA } from "./map";
import { alignmentLength, pointOnAlignment } from "./alignment";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
// Geometry/type tests deliberately have no road reference. Road acquisition and
// its deadline are covered by ConstructionRoadAcceptance, not real network calls.
vi.mock('./roadProvider',()=>({createRoadProvider:()=>({name:'unavailable fixture',load:async()=>{throw Error('road reference unavailable in geometry fixture');}})}));
afterEach(() => { localStorage.removeItem("rosenfu.display-settings"); cleanup(); vi.resetAllMocks(); });
const alignment = { id:"3",owner:"2",name:"試験線",points:[{latitude:35.67504,longitude:139.72},{latitude:35.67504,longitude:139.78}] };
const empty = {next_id:"3",players:[{id:"1",name:"Player"}],companies:[{id:"2",owner:"1",name:"Railway"}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
const built = {...empty,next_id:"4",alignments:[alignment]};
const placed = {...built,next_id:"5",stations:[{id:"4",owner:"2",name:"新駅",latitude:35.67504,longitude:139.75}],station_placements:[{station_id:"4",alignment_id:"3",position:.5,length_m:200,width_m:12}]};
async function start(snapshot: unknown = empty) {
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke).mockImplementation(async command => {if(command==="preview_construction_plan")return {validationError:null};
    if(command==="engine_info") return {name:"Rosenfu",version:"construction-test"};
    if(command==="world_snapshot") return snapshot;
    throw Error("unexpected "+command);
  });
  render(<App/>); fireEvent.click(screen.getByRole("button",{name:"自作サンプル（オフライン）"})); await screen.findByText(/construction-test/);
  const map=screen.getByRole("img",{name:"駅配置用サンプル地図"});
  map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
  return map;
}

const panel=()=>within(document.getElementById("workspace-panel-construction")!);
test("track kind survives independent plans, undo, failed construction and is sent to backend",async()=>{
 const map=await start();const kind=()=>screen.getByRole("button",{name:"高速"});
 expect(kind().getAttribute("aria-pressed")).toBe("false");fireEvent.click(kind());
 await settledMapClick(map,{clientX:180,clientY:130});await settledMapClick(map,{clientX:230,clientY:130});
 fireEvent.click(panel().getByRole("button",{name:"別の線路計画を追加"}));fireEvent.click(screen.getByRole("button",{name:"通常"}));
 await settledMapClick(map,{clientX:320,clientY:200});await settledMapClick(map,{clientX:360,clientY:200});
 fireEvent.click(panel().getByRole("button",{name:/計画 1 を編集/}));expect(kind().getAttribute("aria-pressed")).toBe("true");
 fireEvent.keyDown(document.body,{key:"z",ctrlKey:true});fireEvent.keyDown(document.body,{key:"Z",ctrlKey:true,shiftKey:true});expect(kind().getAttribute("aria-pressed")).toBe("true");
 const original=vi.mocked(invoke).getMockImplementation()!;
 vi.mocked(invoke).mockImplementation(async (command,args)=>{if(command==="preview_construction_plan")return {validationError:null};if(command==="build_alignment")throw Error("kind build rejected");return original(command,args);});fireEvent.click(panel().getByRole("button",{name:"計画を着工"}));await screen.findByText(/kind build rejected/);expect(kind().getAttribute("aria-pressed")).toBe("true");
 expect(vi.mocked(invoke).mock.calls.find(([c])=>c==="build_alignment")?.[1]).toMatchObject({trackType:"high_speed"});
 vi.mocked(invoke).mockImplementation(async (command,args)=>command==="build_alignment"?{...built,alignments:[{...alignment,track_type:"high_speed"}]}:original(command,args));fireEvent.click(panel().getByRole("button",{name:"計画を着工"}));await screen.findByText(/線路を建設しました/);expect(panel().queryByRole("button",{name:"試験線"})).toBeNull();fireEvent.click(screen.getByRole("button",{name:"選択"}));expect(panel().getByRole("button",{name:"試験線"})).toBeTruthy();
});
test("tram defaults road following on without invalidating an existing ordinary draft",async()=>{
 const map=await start();const kind=screen.getByRole("button",{name:"トラム"});
 await settledMapClick(map,{clientX:180,clientY:130});await settledMapClick(map,{clientX:230,clientY:130});
 fireEvent.click(kind);const road=screen.getByRole("button",{name:"道路に合わせる"});expect(road.getAttribute("aria-pressed")).toBe("true");
 fireEvent.click(map,{clientX:300,clientY:130});expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);expect((panel().getByRole("button",{name:"計画を着工"}) as HTMLButtonElement).disabled).toBe(false);
 fireEvent.click(road);expect((panel().getByRole("button",{name:"計画を着工"}) as HTMLButtonElement).disabled).toBe(false);
 vi.mocked(invoke).mockResolvedValueOnce({...built,alignments:[{...alignment,track_type:"tram"}]});fireEvent.click(panel().getByRole("button",{name:"計画を着工"}));await screen.findByText(/線路を建設しました/);
 expect(vi.mocked(invoke).mock.calls.find(([c])=>c==="build_alignment")?.[1]).toMatchObject({trackType:"tram"});
});

test('individual path design speeds survive plan switches and are sent together',async()=>{
 const map=await start(),speed=()=>screen.getByRole('spinbutton',{name:'線路の目標最高速度'}) as HTMLInputElement;
 const click=async (x:number,y:number)=>await settledMapClick(map,{clientX:x,clientY:y,detail:1});
 fireEvent.change(speed(),{target:{value:'95'}});await click(180,130);await click(230,130);await click(230,130);
 await click(300,200);fireEvent.change(speed(),{target:{value:'80'}});await click(360,200);
 fireEvent.click(panel().getByRole('button',{name:'別の線路計画を追加'}));
 fireEvent.change(speed(),{target:{value:'60'}});await click(150,240);await click(200,240);
 fireEvent.click(panel().getByRole('button',{name:/計画 1 を編集/}));expect(speed().value).toBe('95');
 vi.mocked(invoke).mockRejectedValueOnce(Error('retain speed'));fireEvent.click(panel().getByRole('button',{name:'計画を着工'}));await screen.findByText(/retain speed/);
 const requests=vi.mocked(invoke).mock.calls.find(([cmd])=>cmd==='build_plan')![1] as {alignments:{designSpeedKmh:number}[]};
 expect(requests.alignments.map(path=>path.designSpeedKmh).sort()).toEqual([80,95]);
});

test('target speed accepts exact input and adjusts by 20 within existing limits',async()=>{
 await start();const speed=screen.getByLabelText('線路の目標最高速度') as HTMLInputElement;
 const down=screen.getByRole('button',{name:'目標最高速度を20 km/h下げる'}),up=screen.getByRole('button',{name:'目標最高速度を20 km/h上げる'});
 fireEvent.change(speed,{target:{value:'135'}});fireEvent.click(up);expect(speed.value).toBe('155');fireEvent.click(down);expect(speed.value).toBe('135');
 fireEvent.change(speed,{target:{value:'10'}});fireEvent.click(down);expect(speed.value).toBe('1');expect((down as HTMLButtonElement).disabled).toBe(true);
 fireEvent.change(speed,{target:{value:'590'}});fireEvent.click(up);expect(speed.value).toBe('600');expect((up as HTMLButtonElement).disabled).toBe(true);
});


test('parallel mode creates an independent sampled line with undo, redo and persisted geometry',async()=>{
 const map=await start(built);fireEvent.click(screen.getByRole('button',{name:'線路の横に沿わせる'}));
 const point=(longitude:number)=>project(35.67514,longitude,DEFAULT_CAMERA);
 for(const longitude of [139.745,139.755]){const p=point(longitude);await settledMapClick(map,{clientX:p.x/2,clientY:p.y/2,detail:1});}
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 expect((panel().getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 vi.mocked(invoke).mockRejectedValueOnce(Error('retain parallel'));fireEvent.click(panel().getByRole('button',{name:'計画を着工'}));await screen.findByText(/retain parallel/);
 const req=vi.mocked(invoke).mock.calls.find(([cmd])=>cmd==='build_alignment')?.[1] as {points:{latitude:number;longitude:number}[]};
 expect(req.points.length).toBeGreaterThanOrEqual(2);expect(req.points.every(p=>p.latitude>35.67504)).toBe(true);
});

test('cancelling a parallel start never creates an orphan plan; tram leaves parallel mode',async()=>{
 const map=await start(built),p=project(35.67514,139.745,DEFAULT_CAMERA);
 fireEvent.click(screen.getByRole('button',{name:'線路の横に沿わせる'}));await settledMapClick(map,{clientX:p.x/2,clientY:p.y/2,detail:1});
 fireEvent.click(screen.getByRole('button',{name:'トラム'}));
 expect(screen.getByRole('button',{name:'線路の横に沿わせる'}).getAttribute('aria-pressed')).toBe('false');
 expect(screen.getByRole('button',{name:'道路に合わせる'}).getAttribute('aria-pressed')).toBe('true');
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);
 fireEvent.click(screen.getByRole('button',{name:'通常'}));await settledMapClick(map,{clientX:180,clientY:130});await settledMapClick(map,{clientX:230,clientY:130});
 expect((panel().getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
});

test('undo of a transient parallel start leaves the existing draft untouched',async()=>{
 const map=await start(built);await settledMapClick(map,{clientX:180,clientY:130});await settledMapClick(map,{clientX:230,clientY:130});
 const before=map.querySelector('.confirmed-draft')?.getAttribute('d');
 fireEvent.click(screen.getByRole('button',{name:'線路の横に沿わせる'}));const p=project(35.67514,139.745,DEFAULT_CAMERA);await settledMapClick(map,{clientX:p.x/2,clientY:p.y/2,detail:1});
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);expect(map.querySelector('.confirmed-draft')?.getAttribute('d')).toBe(before);
});

 test('all follow modes use accessible custom descriptions without native title tooltips',async()=>{
 await start();
 for(const name of ['実在の路線網に合わせる','道路に合わせる','線路の横に沿わせる']){
  const button=screen.getByRole('button',{name});
  expect(button.hasAttribute('title')).toBe(false);
  expect(button.closest('[title]')).toBeNull();
  expect(button.querySelector('[title]')).toBeNull();
  const tip=document.getElementById(button.getAttribute('aria-describedby')!);
  expect(tip?.getAttribute('role')).toBe('tooltip');
  expect(tip?.classList.contains('follow-tip')).toBe(true);
  expect(tip?.textContent).toContain(name);
  expect(tip?.textContent?.length).toBeGreaterThan(name.length);
 }
});

test('track type selection stays exclusive across all three kinds',async()=>{
 await start();
 const group=screen.getByRole('group',{name:'線路種別'});
 for(const name of ['通常','高速','トラム','通常']){
  fireEvent.click(within(group).getByRole('button',{name}));
  expect(within(group).getAllByRole('button',{pressed:true})).toHaveLength(1);
  expect(within(group).getByRole('button',{pressed:true}).textContent).toBe(name);
 }
});
