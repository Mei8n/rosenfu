import './testConstructionEnvironment';
// @vitest-environment jsdom
import React from "react";
import {afterEach,expect,test,vi} from "vitest";
import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {invoke,isTauri} from "@tauri-apps/api/core";
import {App} from "./App";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn(),isTauri:vi.fn()}));
const empty={next_id:"4",players:[{id:"1",name:"P"}],companies:[{id:"2",owner:"1",name:"C"}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
const alignment={id:"3",owner:"2",name:"線路",points:[{latitude:35.67504,longitude:139.72},{latitude:35.67504,longitude:139.78}]};
const built={...empty,alignments:[alignment]};
HTMLDialogElement.prototype.showModal=function(){this.setAttribute("open","");};
HTMLDialogElement.prototype.close=function(){this.removeAttribute("open");};
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
async function start(snapshot:unknown=built){
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==="engine_info")return {name:"Rosenfu",version:"delete-test"};
  if(command==="world_snapshot")return snapshot;
  if(command==="delete_alignment")return empty;
  if(command==="undo_alignment_deletion")return built;
  throw Error(command);
 });
 render(<App/>);fireEvent.click(screen.getByRole("button",{name:"自作サンプル（オフライン）"}));await screen.findByText(/delete-test/);
 const map=screen.getByRole("img",{name:"駅配置用サンプル地図"});
 map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 return map;
}
test("built line selection, cancel, confirmed deletion and guarded undo",async()=>{
 const map=await start();fireEvent.click(screen.getByRole("button",{name:"選択"}));fireEvent.click(map.querySelector(".alignment-hit")!);
 fireEvent.click(screen.getByRole("button",{name:"選択線路を削除"}));
 expect(invoke).not.toHaveBeenCalledWith("delete_alignment",expect.anything());
 fireEvent.click(screen.getByRole("button",{name:"キャンセル"}));expect(map.querySelector(".alignment-hit")).not.toBeNull();
 fireEvent.click(screen.getByRole("button",{name:"選択線路を削除"}));fireEvent.click(screen.getByRole("button",{name:"線路を削除"}));
 await waitFor(()=>expect(map.querySelector(".alignment-hit")).toBeNull());
 expect(invoke).toHaveBeenCalledWith("delete_alignment",expect.objectContaining({alignmentId:"3",constructionPlans:expect.any(Object)}));
 fireEvent.click(screen.getByRole("button",{name:"線路削除を元に戻す"}));await waitFor(()=>expect(map.querySelector(".alignment-hit")).not.toBeNull());
});
test("station dependency refuses built line deletion",async()=>{
 const map=await start({...built,stations:[{id:"4",owner:"2",name:"駅",latitude:35.67504,longitude:139.75}],station_placements:[{station_id:"4",alignment_id:"3",position:.5,length_m:200,width_m:12}]});
 fireEvent.click(screen.getByRole("button",{name:"選択"}));fireEvent.click(map.querySelector(".alignment-hit")!);
 expect((screen.getByRole("button",{name:"選択線路を削除"}) as HTMLButtonElement).disabled).toBe(true);
 expect(screen.getByText("削除不可：駅が使用中")).toBeTruthy();
});
test("planned line deletion is local, reversible and does not build",async()=>{
 const map=await start(empty);fireEvent.click(map,{clientX:180,clientY:130});fireEvent.click(map,{clientX:230,clientY:130});
 const before=map.querySelector(".confirmed-draft")!.getAttribute("d");
 fireEvent.click(screen.getByRole("button",{name:"選択"}));fireEvent.click(map.querySelector(".plan-hover-hit")!);
 fireEvent.click(screen.getByRole("button",{name:"選択線路を削除"}));fireEvent.click(screen.getByRole("button",{name:"線路を削除"}));
 expect(map.querySelector(".confirmed-draft")).toBeNull();
 fireEvent.click(screen.getByRole("button",{name:"線路削除を元に戻す"}));
 expect(map.querySelector(".confirmed-draft")!.getAttribute("d")).toBe(before);
 expect(vi.mocked(invoke).mock.calls.some(([cmd])=>cmd==="delete_alignment"||cmd==="build_plan")).toBe(false);
});
test("plan station dependency refuses deletion rather than cascading",async()=>{
 const {emptyConstructionPlans}=await import("./constructionPlans");
 const plans={...emptyConstructionPlans(),draftPoints:alignment.points,plannedStations:[{id:"planned:1",pathId:1,planId:1,name:"駅",point:alignment.points[0],lengthM:200}]};
 const map=await start({...empty,construction_plans:plans});
 fireEvent.click(screen.getByRole("button",{name:"選択"}));fireEvent.click(map.querySelector(".plan-hover-hit")!);
 expect((screen.getByRole("button",{name:"選択線路を削除"}) as HTMLButtonElement).disabled).toBe(true);
 expect(screen.getByText("削除不可：計画駅が使用中")).toBeTruthy();
});
test("deleting a parked path preserves current path, saves the removal and supports undo",async()=>{
 const {emptyConstructionPlans}=await import("./constructionPlans");
 const plans={...emptyConstructionPlans(),draftPoints:alignment.points,parkedPaths:[{id:2,planId:1,name:"",points:alignment.points,line:alignment.points,redo:[],trackType:"normal",designSpeedKmh:120,roadFollowing:false,extension:null}]};
 const map=await start({...empty,construction_plans:plans});
 fireEvent.click(screen.getByRole("button",{name:"選択"}));fireEvent.click(map.querySelector('[data-plan-hover-id="2"]')!);
 fireEvent.click(screen.getByRole("button",{name:"選択線路を削除"}));fireEvent.click(screen.getByRole("button",{name:"線路を削除"}));
 expect(map.querySelector(".parked-plan")).toBeNull();expect(map.querySelector(".confirmed-draft")).not.toBeNull();
 vi.mocked(invoke).mockResolvedValueOnce({saved:true});
 fireEvent.click(screen.getByRole("button",{name:"保存"}));await waitFor(()=>expect(invoke).toHaveBeenCalledWith("save_world",expect.objectContaining({constructionPlans:expect.objectContaining({parkedPaths:[]})})));
 fireEvent.click(screen.getByRole("button",{name:"線路削除を元に戻す"}));expect(map.querySelector(".parked-plan")).not.toBeNull();
});

test("planned anchors also select their path in selection mode",async()=>{
 const map=await start(empty);fireEvent.click(map,{clientX:180,clientY:130});fireEvent.click(map,{clientX:230,clientY:130});
 fireEvent.click(screen.getByRole("button",{name:"選択"}));fireEvent.click(map.querySelector(".anchor-drag-hit")!);
 expect((screen.getByRole("button",{name:"選択線路を削除"}) as HTMLButtonElement).disabled).toBe(false);
 expect(map.querySelector(".confirmed-draft")?.classList.contains("track-explicit-selection")).toBe(true);
 expect(vi.mocked(invoke).mock.calls.some(([cmd])=>cmd==="delete_alignment"||cmd==="build_plan")).toBe(false);
});

test("empty-plan persistence cleanup does not invalidate in-session track undo",async()=>{
 const {emptyConstructionPlans}=await import("./constructionPlans");
 const plans={...emptyConstructionPlans(),plans:[{id:1,name:"A"},{id:2,name:"B"}],draftPoints:alignment.points,parkedPaths:[{id:2,planId:2,name:"",points:alignment.points,line:alignment.points,redo:[],trackType:"normal",designSpeedKmh:120,roadFollowing:false,extension:null}]};
 const map=await start({...empty,construction_plans:plans});
 fireEvent.click(screen.getByRole("button",{name:"選択"}));fireEvent.click(map.querySelector('[data-plan-hover-id="2"]')!);
 fireEvent.click(screen.getByRole("button",{name:"選択線路を削除"}));fireEvent.click(screen.getByRole("button",{name:"線路を削除"}));
 expect((screen.getByRole("button",{name:"線路削除を元に戻す"}) as HTMLButtonElement).disabled).toBe(false);
 fireEvent.click(screen.getByRole("button",{name:"線路削除を元に戻す"}));
 expect(map.querySelector(".parked-plan")).not.toBeNull();
});
