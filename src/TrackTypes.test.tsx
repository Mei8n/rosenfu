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

const panel=()=>within(document.getElementById("panel-tracks")!);
test("track kind survives independent plans, undo, failed construction and is sent to backend",async()=>{
 const map=await start();const kind=()=>panel().getByLabelText("線路の種類") as HTMLSelectElement;
 expect(kind().value).toBe("normal");fireEvent.change(kind(),{target:{value:"high_speed"}});
 fireEvent.click(map,{clientX:180,clientY:130});fireEvent.click(map,{clientX:230,clientY:130});
 fireEvent.click(panel().getByRole("button",{name:"別の線路計画を追加"}));fireEvent.change(kind(),{target:{value:"normal"}});
 fireEvent.click(map,{clientX:320,clientY:200});fireEvent.click(map,{clientX:360,clientY:200});
 fireEvent.click(panel().getByRole("button",{name:/計画 1 を編集/}));expect(kind().value).toBe("high_speed");
 fireEvent.keyDown(document.body,{key:"z",ctrlKey:true});fireEvent.keyDown(document.body,{key:"Z",ctrlKey:true,shiftKey:true});expect(kind().value).toBe("high_speed");
 vi.mocked(invoke).mockRejectedValueOnce(Error("kind build rejected"));fireEvent.click(panel().getByRole("button",{name:"計画を着工"}));await screen.findByText(/kind build rejected/);expect(kind().value).toBe("high_speed");
 expect(vi.mocked(invoke).mock.calls.find(([c])=>c==="build_alignment")?.[1]).toMatchObject({trackType:"high_speed"});
 vi.mocked(invoke).mockResolvedValueOnce({...built,alignments:[{...alignment,track_type:"high_speed"}]});fireEvent.click(panel().getByRole("button",{name:"計画を着工"}));await screen.findByText(/線路を建設しました/);expect(panel().getByText(/試験線（高速）/)).toBeTruthy();
});
test("tram defaults road following on, blocks unsupported drawing and construction, permits explicit off",async()=>{
 const map=await start();const kind=panel().getByLabelText("線路の種類");
 fireEvent.click(map,{clientX:180,clientY:130});fireEvent.click(map,{clientX:230,clientY:130});
 fireEvent.change(kind,{target:{value:"tram"}});const road=panel().getByLabelText("道路に合わせる") as HTMLInputElement;expect(road.checked).toBe(true);
 fireEvent.click(map,{clientX:300,clientY:130});expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);expect((panel().getByRole("button",{name:"計画を着工"}) as HTMLButtonElement).disabled).toBe(true);
 fireEvent.click(road);expect((panel().getByRole("button",{name:"計画を着工"}) as HTMLButtonElement).disabled).toBe(false);
 vi.mocked(invoke).mockResolvedValueOnce({...built,alignments:[{...alignment,track_type:"tram"}]});fireEvent.click(panel().getByRole("button",{name:"計画を着工"}));await screen.findByText(/線路を建設しました/);
 expect(vi.mocked(invoke).mock.calls.find(([c])=>c==="build_alignment")?.[1]).toMatchObject({trackType:"tram"});
});
