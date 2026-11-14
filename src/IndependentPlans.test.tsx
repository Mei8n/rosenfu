// @vitest-environment jsdom
import React from "react";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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
    throw Error("unexpected "+command);
  });
  render(<App/>); fireEvent.click(screen.getByRole("button",{name:"自作サンプル（オフライン）"})); await screen.findByText(/construction-test/);
  const map=screen.getByRole("img",{name:"駅配置用サンプル地図"});
  map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
  return map;
}
test("disconnected plans remain visible and build separately without losing the other draft",async()=>{
  const map=await start();
  fireEvent.click(map,{clientX:180,clientY:130});fireEvent.click(map,{clientX:230,clientY:130});
  const first=map.querySelector(".draft-path")!.getAttribute("d");
  fireEvent.click(trackPanel().getByRole("button",{name:"別の線路計画を追加"}));
  expect(map.querySelectorAll(".parked-plan")).toHaveLength(1);
  fireEvent.click(map,{clientX:320,clientY:200});fireEvent.click(map,{clientX:360,clientY:200});
  const second=map.querySelector(".draft-path:not(.parked-plan)")!.getAttribute("d");
  expect(second).not.toBe(first);
  fireEvent.click(trackPanel().getByRole("button",{name:/計画 1 を編集/}));
  expect(map.querySelector(".draft-path:not(.parked-plan)")!.getAttribute("d")).toBe(first);
  expect(map.querySelector(".parked-plan")!.getAttribute("d")).toBe(second);
  vi.mocked(invoke).mockRejectedValueOnce(Error("build rejected"));
  fireEvent.click(trackPanel().getByRole("button",{name:"計画を着工"}));await screen.findByText(/build rejected/);
  expect(map.querySelectorAll(".draft-path")).toHaveLength(2);
  vi.mocked(invoke).mockResolvedValueOnce(built);
  fireEvent.click(trackPanel().getByRole("button",{name:"計画を着工"}));await screen.findByText(/線路を建設しました/);
  expect(map.querySelectorAll(".parked-plan")).toHaveLength(0);
  expect((screen.getByRole("button",{name:"保存"}) as HTMLButtonElement).disabled).toBe(false);
  expect(trackPanel().queryByRole("group",{name:"計画を選択"})).toBeNull();
  expect(map.querySelector(".draft-path:not(.parked-plan)")!.getAttribute("d")).toBe(second);
  expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
  fireEvent.keyDown(document.body,{key:"z",ctrlKey:true});expect(map.querySelectorAll(".draft-anchor")).toHaveLength(1);
  fireEvent.keyDown(document.body,{key:"Z",ctrlKey:true,shiftKey:true});expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
});
test("clearing active plan does not erase parked plans and loading asks before discarding",async()=>{
  const map=await start();
  fireEvent.click(map,{clientX:180,clientY:130});fireEvent.click(map,{clientX:230,clientY:130});
  fireEvent.click(trackPanel().getByRole("button",{name:"別の線路計画を追加"}));
  fireEvent.click(map,{clientX:320,clientY:200});
  fireEvent.click(trackPanel().getByRole("button",{name:"下書きを消去"}));
  expect(map.querySelectorAll(".parked-plan")).toHaveLength(1);
  fireEvent.click(screen.getByRole("button",{name:"読込"}));
  expect(screen.getByRole("button",{name:"読込を続ける"})).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"戻る"}));
  expect(map.querySelectorAll(".parked-plan")).toHaveLength(1);
});

test("plan names are optional, editable per independent plan and never become physical track names",async()=>{
 const map=await start();const name=()=>trackPanel().getByLabelText("計画名（任意）") as HTMLInputElement;
 expect(name().value).toBe("");fireEvent.change(name(),{target:{value:"海沿い案"}});
 fireEvent.click(map,{clientX:180,clientY:130});fireEvent.click(map,{clientX:230,clientY:130});
 fireEvent.click(trackPanel().getByRole("button",{name:"別の線路計画を追加"}));expect(name().value).toBe("");
 fireEvent.click(map,{clientX:320,clientY:200});fireEvent.click(map,{clientX:360,clientY:200});
 fireEvent.click(trackPanel().getByRole("button",{name:/海沿い案 を編集/}));expect(name().value).toBe("海沿い案");
 fireEvent.change(name(),{target:{value:"山側に変更"}});
 fireEvent.click(trackPanel().getByRole("button",{name:/計画 2 を編集/}));expect(name().value).toBe("");
 expect(trackPanel().getByRole("button",{name:/山側に変更 を編集/})).toBeTruthy();
 vi.mocked(invoke).mockResolvedValueOnce(built);fireEvent.click(trackPanel().getByRole("button",{name:"計画を着工"}));await screen.findByText(/線路を建設しました/);
 const payload=vi.mocked(invoke).mock.calls.find(([command])=>command==="build_alignment")?.[1] as {name:string};expect(payload.name).toBe("新しい線路");
 await waitFor(()=>expect(name().value).toBe("山側に変更"));fireEvent.change(name(),{target:{value:""}});expect(name().placeholder).toBe("計画 1");
});

test("manual save includes plans and retains active, parked and redo; later edits require a load guard",async()=>{
 const map=await start(built);fireEvent.click(map,{clientX:180,clientY:130});fireEvent.click(map,{clientX:230,clientY:130});
 fireEvent.click(trackPanel().getByRole("button",{name:"別の線路計画を追加"}));fireEvent.click(map,{clientX:320,clientY:200});fireEvent.click(map,{clientX:360,clientY:200});
 fireEvent.keyDown(document.body,{key:"z",ctrlKey:true});const paths=Array.from(map.querySelectorAll('.draft-path')).map(p=>p.getAttribute('d'));
 vi.mocked(invoke).mockRejectedValueOnce(Error('save rejected'));fireEvent.click(screen.getByRole('button',{name:'保存'}));await screen.findByText(/save rejected/);expect(Array.from(map.querySelectorAll('.draft-path')).map(p=>p.getAttribute('d'))).toEqual(paths);
 vi.mocked(invoke).mockResolvedValueOnce({saved:true});fireEvent.click(screen.getByRole('button',{name:'保存'}));await screen.findByText(/ワールドと未着工計画を保存しました/);expect(Array.from(map.querySelectorAll('.draft-path')).map(p=>p.getAttribute('d'))).toEqual(paths);
 await waitFor(()=>expect((screen.getByRole('button',{name:'保存'}) as HTMLButtonElement).disabled).toBe(false));fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 fireEvent.click(screen.getByRole('button',{name:'読込'}));expect(screen.getByRole('button',{name:'読込を続ける'})).toBeTruthy();
});

test("one explicit plan groups disconnected paths and atomically builds only its paths and stations",async()=>{
 const map=await start();
 const click=(x:number,y:number)=>fireEvent.click(map,{clientX:x,clientY:y,detail:1});
 const station=(x:number,y:number)=>{fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));click(x,y);fireEvent.click(screen.getByRole("tab",{name:"線路"}));};
 click(180,130);click(230,130);station(205,130);click(230,130);
 click(320,200);click(360,200);station(340,200);
 expect(trackPanel().getByText(/2経路 ·/)).toBeTruthy();
 expect(trackPanel().queryByRole("button",{name:/計画 2 を編集/})).toBeNull();
 expect(map.querySelectorAll(".planned-station")).toHaveLength(2);
 fireEvent.click(trackPanel().getByRole("button",{name:"別の線路計画を追加"}));click(180,240);click(230,240);station(205,240);
 fireEvent.click(trackPanel().getByRole("button",{name:/計画 1 を編集/}));
 vi.mocked(invoke).mockRejectedValueOnce(Error("batch rejected"));fireEvent.click(trackPanel().getByRole("button",{name:"計画を着工"}));await screen.findByText(/batch rejected/);
 expect(map.querySelectorAll(".planned-station")).toHaveLength(3);
 const request=vi.mocked(invoke).mock.calls.find(([command])=>command==="build_plan")?.[1] as {alignments:{stations:unknown[]}[]};
 expect(request.alignments).toHaveLength(2);expect(request.alignments.map(path=>path.stations.length)).toEqual([1,1]);
 vi.mocked(invoke).mockResolvedValueOnce(built);fireEvent.click(trackPanel().getByRole("button",{name:"計画を着工"}));await screen.findByText(/線路を建設しました/);
 expect(map.querySelectorAll(".planned-station")).toHaveLength(1);expect(map.querySelectorAll(".parked-plan")).toHaveLength(0);
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
});

test("resuming a parked path preserves its point redo and station within the same plan",async()=>{
 const map=await start();const click=(x:number,y:number)=>fireEvent.click(map,{clientX:x,clientY:y,detail:1});
 click(180,130);click(230,130);fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));click(205,130);fireEvent.click(screen.getByRole("tab",{name:"線路"}));click(230,130);
 click(320,200);click(360,200);click(360,200);click(230,130);
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);expect(map.querySelectorAll(".planned-station")).toHaveLength(1);
 click(280,130);expect(map.querySelectorAll(".draft-anchor")).toHaveLength(3);
 fireEvent.keyDown(document.body,{key:"z",ctrlKey:true});fireEvent.keyDown(document.body,{key:"Z",ctrlKey:true,shiftKey:true});expect(map.querySelectorAll(".draft-anchor")).toHaveLength(3);
 expect(trackPanel().getByText(/2経路 ·/)).toBeTruthy();
 fireEvent.click(trackPanel().getByRole("button",{name:"下書きを消去"}));expect(map.querySelectorAll(".planned-station")).toHaveLength(0);expect(map.querySelectorAll(".draft-path")).toHaveLength(0);
});

test("a parked redo-only path remains selectable and can recover without deleting its plan",async()=>{
 const map=await start();const click=(x:number,y:number)=>fireEvent.click(map,{clientX:x,clientY:y,detail:1});
 click(180,130);click(230,130);click(230,130);click(320,200);
 fireEvent.keyDown(document.body,{key:"z",ctrlKey:true});click(230,130);
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
 fireEvent.click(screen.getByText("内訳"));fireEvent.click(screen.getByRole("button",{name:"経路 2"}));
 fireEvent.keyDown(document.body,{key:"Z",ctrlKey:true,shiftKey:true});expect(map.querySelectorAll(".draft-anchor")).toHaveLength(1);
 click(360,200);expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
 expect((screen.getByRole("button",{name:"計画を着工"}) as HTMLButtonElement).disabled).toBe(false);
});

test("manual round-trip restores multiple plans, paths, stations, redo and independent IDs",async()=>{
 const map=await start();const click=(x:number,y:number)=>fireEvent.click(map,{clientX:x,clientY:y,detail:1});
 click(180,130);click(230,130);fireEvent.click(screen.getByRole('tab',{name:'駅設置'}));click(205,130);fireEvent.click(screen.getByRole('tab',{name:'線路'}));click(230,130);
 click(320,200);click(360,200);
 fireEvent.click(trackPanel().getByRole('button',{name:'別の線路計画を追加'}));
 fireEvent.change(trackPanel().getByLabelText('計画名（任意）'),{target:{value:'第二計画'}});
 click(180,240);click(230,240);fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 vi.mocked(invoke).mockResolvedValueOnce({saved:true});fireEvent.click(screen.getByRole('button',{name:'保存'}));await screen.findByText(/ワールドと未着工計画を保存しました/);
 const saved=(vi.mocked(invoke).mock.calls.find(([c])=>c==='save_world')![1] as {constructionPlans:import('./constructionPlans').ConstructionPlans}).constructionPlans;
 expect(saved.plans).toHaveLength(2);expect(saved.parkedPaths).toHaveLength(2);expect(saved.plannedStations).toHaveLength(1);expect(saved.redoPoints).toHaveLength(1);
 await waitFor(()=>expect((screen.getByRole('button',{name:'保存'}) as HTMLButtonElement).disabled).toBe(false));
 const beforeUnload=new Event('beforeunload',{cancelable:true});window.dispatchEvent(beforeUnload);expect(beforeUnload.defaultPrevented).toBe(false);
 fireEvent.click(trackPanel().getByRole('button',{name:'下書きを消去'}));
 vi.mocked(invoke).mockResolvedValueOnce({...empty,construction_plans:saved});fireEvent.click(screen.getByRole('button',{name:'読込'}));fireEvent.click(screen.getByRole('button',{name:'読込を続ける'}));
 await waitFor(()=>expect((trackPanel().getByLabelText('計画名（任意）') as HTMLInputElement).value).toBe('第二計画'));
 expect(map.querySelectorAll('.parked-plan')).toHaveLength(2);expect(map.querySelectorAll('.planned-station')).toHaveLength(1);
 await waitFor(()=>expect((screen.getByRole('button',{name:'保存'}) as HTMLButtonElement).disabled).toBe(false));
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 fireEvent.click(trackPanel().getByRole('button',{name:'別の線路計画を追加'}));expect((trackPanel().getByLabelText("計画名（任意）") as HTMLInputElement).placeholder).toBe("計画 3");
});

test('built-line stations remain separate plans until atomic construction, and persist without new tracks',async()=>{
 const map=await start(built);fireEvent.click(screen.getByRole('tab',{name:'駅設置'}));
 fireEvent.click(map,{clientX:230,clientY:150});fireEvent.click(map,{clientX:280,clientY:150});
 expect(map.querySelectorAll('.planned-station')).toHaveLength(2);
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='place_station_on_alignment'||c==='build_plan')).toBe(false);
 fireEvent.click(screen.getByRole('button',{name:'別の線路計画を追加'}));fireEvent.click(map,{clientX:250,clientY:150});expect(map.querySelectorAll('.planned-station')).toHaveLength(3);
 vi.mocked(invoke).mockResolvedValueOnce({saved:true});fireEvent.click(screen.getByRole('button',{name:'保存'}));await screen.findByText(/ワールドと未着工計画を保存しました/);
 const saved=(vi.mocked(invoke).mock.calls.find(([c])=>c==='save_world')![1] as {constructionPlans:import('./constructionPlans').ConstructionPlans}).constructionPlans;
 expect(saved.parkedPaths).toHaveLength(0);expect(saved.plannedStations.map(s=>s.planId)).toEqual([1,1,2]);expect(saved.plannedStations.every(s=>s.alignmentId==='3'&&s.pathId===undefined)).toBe(true);
 await waitFor(()=>expect((screen.getByRole('button',{name:'保存'}) as HTMLButtonElement).disabled).toBe(false));
 vi.mocked(invoke).mockResolvedValueOnce({...built,construction_plans:saved});fireEvent.click(screen.getByRole('button',{name:'読込'}));await screen.findByText('保存したワールドを読み込みました。');await waitFor(()=>expect((screen.getByRole('button',{name:'保存'}) as HTMLButtonElement).disabled).toBe(false));
 fireEvent.click(screen.getByRole('button',{name:'計画 1 を編集'}));
 vi.mocked(invoke).mockRejectedValueOnce(Error('station batch rejected'));fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));await screen.findByText(/station batch rejected/);expect(map.querySelectorAll('.planned-station')).toHaveLength(3);
 const request=vi.mocked(invoke).mock.calls.find(([c])=>c==='build_plan')![1] as {alignments:unknown[];stations:unknown[]};expect(request.alignments).toHaveLength(0);expect(request.stations).toHaveLength(2);
 await waitFor(()=>expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false));vi.mocked(invoke).mockResolvedValueOnce(placed);fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));await screen.findByText('計画駅を建設しました。');expect(map.querySelectorAll('.planned-station')).toHaveLength(1);
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
});

test('construction shares one plan panel between track and station tools, and help lives in settings',async()=>{
 await start();
 expect(screen.getAllByRole('region',{name:'建設計画'})).toHaveLength(1);
 expect(screen.queryByText('操作ガイド')?.closest('[hidden]')).toBeTruthy();
 fireEvent.change(screen.getByLabelText('計画名（任意）'),{target:{value:'共通案'}});
 fireEvent.click(screen.getByRole('tab',{name:'駅設置'}));
 expect((screen.getByLabelText('計画名（任意）') as HTMLInputElement).value).toBe('共通案');
 expect(screen.getAllByRole('button',{name:'計画を着工'})).toHaveLength(1);
 fireEvent.click(screen.getByRole('tab',{name:'設定'}));
 expect(screen.getByText('操作ガイド').closest('[hidden]')).toBeNull();
});

test("completed plans do not reappear as empty plan switcher entries after saving and restarting",async()=>{
 const map=await start();
 for(const [x,y] of [[180,130],[230,130]])fireEvent.click(map,{clientX:x,clientY:y});
 fireEvent.click(trackPanel().getByRole('button',{name:'別の線路計画を追加'}));
 for(const [x,y] of [[320,200],[360,200]])fireEvent.click(map,{clientX:x,clientY:y});
 for(let i=0;i<2;i++){
  vi.mocked(invoke).mockResolvedValueOnce(built);
  fireEvent.click(trackPanel().getByRole('button',{name:'計画を着工'}));
  await waitFor(()=>expect((screen.getByRole('button',{name:'保存'}) as HTMLButtonElement).disabled).toBe(false));
 }
 vi.mocked(invoke).mockResolvedValueOnce({saved:true});fireEvent.click(screen.getByRole('button',{name:'保存'}));
 await screen.findByText(/ワールドと未着工計画を保存しました/);
 const payload=vi.mocked(invoke).mock.calls.find(([c])=>c==='save_world')![1] as {constructionPlans:unknown};
 cleanup();await start({...built,construction_plans:payload.constructionPlans});
 expect(trackPanel().queryByRole('group',{name:'計画を選択'})).toBeNull();
});

test('switching plans stays finished until an explicit endpoint click, including save restore',async()=>{
 const map=await start();const click=(x:number,y:number)=>fireEvent.click(map,{clientX:x,clientY:y,detail:1});
 click(180,130);click(230,130);fireEvent.click(trackPanel().getByRole('button',{name:'別の線路計画を追加'}));click(320,200);click(360,200);
 fireEvent.click(trackPanel().getByRole('button',{name:'計画 1 を編集'}));
 vi.mocked(invoke).mockResolvedValueOnce({saved:true});fireEvent.click(screen.getByRole('button',{name:'保存'}));await screen.findByText(/ワールドと未着工計画を保存しました/);
 const saved=(vi.mocked(invoke).mock.calls.find(([c])=>c==='save_world')![1] as {constructionPlans:import('./constructionPlans').ConstructionPlans}).constructionPlans;
 expect(saved.draftFinished).toBe(true);
 cleanup();const restored=await start({...empty,construction_plans:saved});
 fireEvent.click(restored,{clientX:230,clientY:130,detail:1});fireEvent.click(restored,{clientX:280,clientY:130,detail:1});expect(restored.querySelectorAll('.draft-anchor')).toHaveLength(3);
});

test('deleting a plan asks first and preserves other plans and built infrastructure',async()=>{
 const map=await start(built);const click=(x:number,y:number)=>fireEvent.click(map,{clientX:x,clientY:y});
 click(180,130);click(230,130);fireEvent.click(trackPanel().getByRole('button',{name:'別の線路計画を追加'}));click(320,200);click(360,200);
 fireEvent.click(trackPanel().getByRole('button',{name:'計画を削除'}));
 let dialog=screen.getByRole('dialog',{name:'計画を削除'});fireEvent.click(within(dialog).getByRole('button',{name:'戻る'}));expect(map.querySelectorAll('.draft-path')).toHaveLength(2);
 fireEvent.click(trackPanel().getByRole('button',{name:'計画を削除'}));dialog=screen.getByRole('dialog',{name:'計画を削除'});fireEvent.click(within(dialog).getByRole('button',{name:'削除する'}));
 expect(map.querySelectorAll('.draft-path')).toHaveLength(1);expect(trackPanel().queryByRole('group',{name:'計画を選択'})).toBeNull();expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='delete_alignment')).toBe(false);
});
