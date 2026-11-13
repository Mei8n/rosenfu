// @vitest-environment jsdom
import React from "react";
import {afterEach,expect,test,vi} from "vitest";
import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {invoke,isTauri} from "@tauri-apps/api/core";
import {App} from "./App";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn(),isTauri:vi.fn()}));
window.PointerEvent=MouseEvent as typeof PointerEvent;
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
const empty={next_id:"3",players:[{id:"1",name:"Player"}],companies:[{id:"2",owner:"1",name:"Railway"}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
async function start(){
 vi.mocked(isTauri).mockReturnValue(true);vi.mocked(invoke).mockImplementation(async command=>{
 if(command==="engine_info")return {name:"Rosenfu",version:"direct-test"};
 if(command==="world_snapshot"||command==="load_world")return empty;
 if(command==="save_world")return {saved:true};
 throw Error("unexpected "+command);
 });render(<App/>);await screen.findByText(/direct-test/);
 const map=screen.getByRole("img",{name:"駅配置用OpenStreetMap"});
 map.getBoundingClientRect=()=>({left:0,top:0,right:500,bottom:300,x:0,y:0,width:500,height:300,toJSON:()=>({})});
 fireEvent.click(map,{clientX:180,clientY:150});fireEvent.click(map,{clientX:320,clientY:150});
 fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));return map;
}
test("direct hover and click places planned stations without committing tracks; repeated placement, undo, tabs and save keep plans",async()=>{
 const map=await start();
 expect(screen.queryByLabelText("配置先の線路")).toBeNull();
 expect(screen.queryByRole("button",{name:"ホームを配置"})).toBeNull();
 fireEvent.pointerMove(map,{clientX:230,clientY:150});
 expect(map.querySelector(".station-platform")).toBeTruthy();
 expect(map.querySelectorAll(".planned-station")).toHaveLength(0);
 fireEvent.click(map,{clientX:230,clientY:150});fireEvent.click(map,{clientX:280,clientY:150});
 expect(map.querySelectorAll(".planned-station")).toHaveLength(2);
 expect(invoke).not.toHaveBeenCalledWith("place_station_on_alignment",expect.anything());
 expect(map.classList.contains("placement-enabled")).toBe(true);
 fireEvent.keyDown(document.body,{key:"z",ctrlKey:true});expect(map.querySelectorAll(".planned-station")).toHaveLength(1);
 fireEvent.keyDown(document.body,{key:"z",ctrlKey:true,shiftKey:true});expect(map.querySelectorAll(".planned-station")).toHaveLength(2);
 fireEvent.click(screen.getByRole("tab",{name:"路線"}));expect(map.querySelectorAll(".planned-station")).toHaveLength(2);
 fireEvent.click(screen.getByRole("button",{name:"保存"}));await screen.findByText(/建設済みワールドを保存しました/);
 expect(map.querySelectorAll(".planned-station")).toHaveLength(2);
 fireEvent.click(screen.getByRole("button",{name:"読込"}));fireEvent.click(screen.getByRole("button",{name:"読込を続ける"}));await screen.findByText("保存したワールドを読み込みました。");
 expect(map.querySelectorAll(".planned-station")).toHaveLength(0);
});
test("atomic plan construction submits stations together; failure preserves draft and success clears it",async()=>{
 const map=await start();fireEvent.click(map,{clientX:250,clientY:150});
 fireEvent.click(screen.getByRole("tab",{name:"線路"}));
 vi.mocked(invoke).mockRejectedValueOnce(Error("atomic failure"));
 fireEvent.click(screen.getByRole("button",{name:"計画を着工"}));await screen.findByText(/atomic failure/);
 expect(map.querySelectorAll(".planned-station")).toHaveLength(1);expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
 expect(invoke).toHaveBeenCalledWith("build_alignment_with_stations",expect.objectContaining({stations:[expect.objectContaining({name:"新駅",lengthM:200,widthM:12})]}));
 vi.mocked(invoke).mockResolvedValueOnce(empty);fireEvent.click(screen.getByRole("button",{name:"計画を着工"}));await screen.findByText(/線路を建設しました/);
 expect(map.querySelectorAll(".planned-station")).toHaveLength(0);
});
test("drag does not place a station and clearing one independent plan only removes its stations",async()=>{
 const map=await start();
 fireEvent.pointerDown(map,{clientX:240,clientY:150,button:0,pointerId:1});fireEvent.pointerMove(map,{clientX:250,clientY:150,pointerId:1});fireEvent.pointerUp(map,{clientX:250,clientY:150,pointerId:1});fireEvent.click(map,{clientX:250,clientY:150});
 expect(map.querySelectorAll(".planned-station")).toHaveLength(0);
 fireEvent.click(screen.getByRole("tab",{name:"線路"}));fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));fireEvent.click(map,{clientX:250,clientY:150});
 expect(map.querySelectorAll(".planned-station")).toHaveLength(1);
 fireEvent.click(screen.getByRole("tab",{name:"線路"}));fireEvent.click(screen.getByRole("button",{name:"別の線路計画を追加"}));
 fireEvent.click(map,{clientX:180,clientY:180});fireEvent.click(map,{clientX:320,clientY:180});
 fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));fireEvent.click(map,{clientX:250,clientY:180});expect(map.querySelectorAll(".planned-station")).toHaveLength(2);
 fireEvent.click(screen.getByRole("tab",{name:"線路"}));fireEvent.click(screen.getByRole("button",{name:"下書きを消去"}));
 expect(map.querySelectorAll(".planned-station")).toHaveLength(1);
 await waitFor(()=>expect(screen.getByText("新駅（計画）")).toBeTruthy());
});
test("changing a plan so its station no longer fits blocks construction until station is removed",async()=>{
 const map=await start();fireEvent.click(map,{clientX:310,clientY:150});
 fireEvent.click(screen.getByRole("tab",{name:"線路"}));fireEvent.click(screen.getByRole("button",{name:"1点戻す"}));
 fireEvent.click(map,{clientX:250,clientY:150});fireEvent.click(screen.getByRole("button",{name:"計画を着工"}));
 await screen.findByText(/計画駅が線路に収まりません/);
 expect(invoke).not.toHaveBeenCalledWith("build_alignment_with_stations",expect.anything());
 expect(invoke).not.toHaveBeenCalledWith("build_alignment",expect.anything());
 fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));fireEvent.click(screen.getByRole("button",{name:"計画駅 新駅 を削除"}));
 expect(map.querySelectorAll(".planned-station")).toHaveLength(0);
});
