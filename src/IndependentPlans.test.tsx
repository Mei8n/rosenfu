// @vitest-environment jsdom
import React from "react";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { App } from "./App";
import { project, DEFAULT_CAMERA } from "./map";
import { alignmentLength, pointOnAlignment } from "./alignment";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
afterEach(() => { localStorage.removeItem("rosenfu.display-settings"); cleanup(); vi.resetAllMocks(); });
const alignment = { id:"3",owner:"2",name:"試験線",points:[{latitude:35.67504,longitude:139.72},{latitude:35.67504,longitude:139.78}] };
const trackPanel = () => within(document.querySelector<HTMLElement>(".track-editor")!);
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
  expect(map.querySelectorAll(".parked-plan")).toHaveLength(1);
  expect((screen.getByRole("button",{name:"保存"}) as HTMLButtonElement).disabled).toBe(false);
  fireEvent.click(trackPanel().getByRole("button",{name:/計画 2 を編集/}));
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
 await waitFor(()=>expect((trackPanel().getByRole("button",{name:/山側に変更 を編集/}) as HTMLButtonElement).disabled).toBe(false));fireEvent.click(trackPanel().getByRole("button",{name:/山側に変更 を編集/}));fireEvent.change(name(),{target:{value:""}});expect(trackPanel().getByText(/編集中：計画 1/)).toBeTruthy();
});

test("manual save excludes plans but retains active, parked, redo and close/load guards",async()=>{
 const map=await start(built);fireEvent.click(map,{clientX:180,clientY:130});fireEvent.click(map,{clientX:230,clientY:130});
 fireEvent.click(trackPanel().getByRole("button",{name:"別の線路計画を追加"}));fireEvent.click(map,{clientX:320,clientY:200});fireEvent.click(map,{clientX:360,clientY:200});
 fireEvent.keyDown(document.body,{key:"z",ctrlKey:true});const paths=Array.from(map.querySelectorAll('.draft-path')).map(p=>p.getAttribute('d'));
 vi.mocked(invoke).mockRejectedValueOnce(Error('save rejected'));fireEvent.click(screen.getByRole('button',{name:'保存'}));await screen.findByText(/save rejected/);expect(Array.from(map.querySelectorAll('.draft-path')).map(p=>p.getAttribute('d'))).toEqual(paths);
 vi.mocked(invoke).mockResolvedValueOnce({saved:true});fireEvent.click(screen.getByRole('button',{name:'保存'}));await screen.findByText(/建設済みワールドを保存しました/);expect(Array.from(map.querySelectorAll('.draft-path')).map(p=>p.getAttribute('d'))).toEqual(paths);
 await waitFor(()=>expect((screen.getByRole('button',{name:'保存'}) as HTMLButtonElement).disabled).toBe(false));fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 fireEvent.click(screen.getByRole('button',{name:'読込'}));expect(screen.getByRole('button',{name:'読込を続ける'})).toBeTruthy();
});

test("one explicit plan groups disconnected paths and atomically builds only its paths and stations",async()=>{
 const map=await start();
 const click=(x:number,y:number)=>fireEvent.click(map,{clientX:x,clientY:y,detail:1});
 const station=(x:number,y:number)=>{fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));click(x,y);fireEvent.click(screen.getByRole("tab",{name:"線路"}));};
 click(180,130);click(230,130);station(205,130);click(230,130);
 click(320,200);click(360,200);station(340,200);
 expect(trackPanel().getByText(/編集中：計画 1（2経路）/)).toBeTruthy();
 expect(trackPanel().queryByRole("button",{name:/計画 2 を編集/})).toBeNull();
 expect(map.querySelectorAll(".planned-station")).toHaveLength(2);
 fireEvent.click(trackPanel().getByRole("button",{name:"別の線路計画を追加"}));click(180,240);click(230,240);station(205,240);
 fireEvent.click(trackPanel().getByRole("button",{name:/計画 1 を編集/}));
 vi.mocked(invoke).mockRejectedValueOnce(Error("batch rejected"));fireEvent.click(trackPanel().getByRole("button",{name:"計画を着工"}));await screen.findByText(/batch rejected/);
 expect(map.querySelectorAll(".planned-station")).toHaveLength(3);
 const request=vi.mocked(invoke).mock.calls.find(([command])=>command==="build_plan")?.[1] as {alignments:{stations:unknown[]}[]};
 expect(request.alignments).toHaveLength(2);expect(request.alignments.map(path=>path.stations.length)).toEqual([1,1]);
 vi.mocked(invoke).mockResolvedValueOnce(built);fireEvent.click(trackPanel().getByRole("button",{name:"計画を着工"}));await screen.findByText(/線路を建設しました/);
 expect(map.querySelectorAll(".planned-station")).toHaveLength(1);expect(map.querySelectorAll(".parked-plan")).toHaveLength(1);
 fireEvent.click(trackPanel().getByRole("button",{name:/計画 2 を編集/}));expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
});

test("resuming a parked path preserves its point redo and station within the same plan",async()=>{
 const map=await start();const click=(x:number,y:number)=>fireEvent.click(map,{clientX:x,clientY:y,detail:1});
 click(180,130);click(230,130);fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));click(205,130);fireEvent.click(screen.getByRole("tab",{name:"線路"}));click(230,130);
 click(320,200);click(360,200);click(360,200);click(230,130);
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);expect(map.querySelectorAll(".planned-station")).toHaveLength(1);
 click(280,130);expect(map.querySelectorAll(".draft-anchor")).toHaveLength(3);
 fireEvent.keyDown(document.body,{key:"z",ctrlKey:true});fireEvent.keyDown(document.body,{key:"Z",ctrlKey:true,shiftKey:true});expect(map.querySelectorAll(".draft-anchor")).toHaveLength(3);
 expect(trackPanel().getByText(/編集中：計画 1（2経路）/)).toBeTruthy();
 fireEvent.click(trackPanel().getByRole("button",{name:"下書きを消去"}));expect(map.querySelectorAll(".planned-station")).toHaveLength(0);expect(map.querySelectorAll(".draft-path")).toHaveLength(0);
});
