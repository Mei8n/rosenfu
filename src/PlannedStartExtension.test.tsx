// @vitest-environment jsdom
import React from "react";
import {afterEach,expect,test,vi} from "vitest";
import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {invoke,isTauri} from "@tauri-apps/api/core";
import {App} from "./App";
import {emptyConstructionPlans} from "./constructionPlans";
import type {ConstructionPlans} from "./constructionPlans";
import {circularAlignment} from "./circularAlignment";
import {coordinatesFromClient,DEFAULT_CAMERA} from "./map";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn(),isTauri:vi.fn()}));
window.PointerEvent=MouseEvent as typeof PointerEvent;
afterEach(()=>{cleanup();vi.resetAllMocks();localStorage.clear();});
const rect={x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})};
const point=(x:number,y:number)=>coordinatesFromClient(x,y,rect,DEFAULT_CAMERA)!;
const line={id:"3",owner:"2",name:"既存",track_type:"normal",points:[point(180,130),point(250,130)]};
const world={next_id:"4",players:[{id:"1",name:"P"}],companies:[{id:"2",owner:"1",name:"C"}],stations:[],tracks:[],routes:[],services:[],alignments:[line],station_placements:[]};
async function start(extra={}){
 vi.mocked(isTauri).mockReturnValue(true);vi.mocked(invoke).mockImplementation(async command=>{
 if(command==="engine_info")return {name:"Rosenfu",version:"extension-test"};if(command==="world_snapshot")return {...world,...extra};throw Error("unexpected "+command);
 });
 // Resolve the mocked startup IPC and restored-plan effects before the first key.
 // Version text alone can render before the window keyboard listener is refreshed.
 await act(async()=>{render(<App/>);});
 fireEvent.click(screen.getByRole("button",{name:"自作サンプル（オフライン）"}));expect(screen.getByText(/extension-test/)).toBeTruthy();
 const map=screen.getByRole("img",{name:"駅配置用サンプル地図"});map.getBoundingClientRect=()=>rect;return map;
}

test("completed planned start resumes the same path without a duplicate anchor",async()=>{
 const map=await start({alignments:[]});const click=(x:number,y=200)=>fireEvent.click(map,{clientX:x,clientY:y,detail:1});
 click(160);click(260);click(260);
 click(162);
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
 click(90);click(90);
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(3);
 expect(map.querySelectorAll(".parked-plan")).toHaveLength(0);
 expect((screen.getByRole("button",{name:"計画を着工"}) as HTMLButtonElement).disabled).toBe(false);
 fireEvent.keyDown(document.body,{key:"z",ctrlKey:true});
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
 fireEvent.keyDown(document.body,{key:"Z",ctrlKey:true,shiftKey:true});
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(3);
});
test("parked planned start resumes the same path",async()=>{
 const plans={...emptyConstructionPlans(),draftPoints:[point(100,80),point(200,80)],draftFinished:true,
  parkedPaths:[{id:2,planId:1,name:"",points:[point(160,200),point(260,200)],redo:[],line:[point(160,200),point(260,200)],designSpeedKmh:120,trackType:"normal",roadFollowing:false,extension:null}]};
 const map=await start({alignments:[],construction_plans:plans});
 fireEvent.click(map,{clientX:162,clientY:200,detail:1});
 fireEvent.click(map,{clientX:90,clientY:200,detail:1});
 expect(map.querySelectorAll(".draft-path.parked-plan")).toHaveLength(1);
 expect((screen.getByRole("button",{name:"計画を着工"}) as HTMLButtonElement).disabled).toBe(false);
 vi.mocked(invoke).mockResolvedValueOnce({saved:true});fireEvent.click(screen.getByRole("button",{name:"保存"}));
 await waitFor(()=>expect(vi.mocked(invoke).mock.calls.some(c=>c[0]==="save_world")).toBe(true));
 const saved=vi.mocked(invoke).mock.calls.find(c=>c[0]==="save_world")![1] as {constructionPlans:{activePathId:number;draftPoints:unknown[]}};
 expect(saved.constructionPlans.activePathId).toBe(2);
 expect(saved.constructionPlans.draftPoints).toEqual([point(260,200),point(160,200),point(90,200)]);
});
test("start resumption preserves sampled curve and station coordinates through save and direction Undo",async()=>{
 const anchors=[point(100,200),point(220,200),point(300,150)];
 const curve=circularAlignment(anchors).points;
 const station={id:"planned:1",pathId:1,planId:1,name:"保持駅",point:point(160,200),lengthM:40};
 const map=await start({alignments:[],construction_plans:{...emptyConstructionPlans(),draftPoints:anchors,draftFinished:true,plannedStations:[station]}});
 const save=async()=>{
  vi.mocked(invoke).mockResolvedValueOnce({saved:true});
  await act(async()=>{fireEvent.click(screen.getByRole("button",{name:"保存"}));});
  await waitFor(()=>expect(vi.mocked(invoke).mock.calls.at(-1)?.[0]).toBe("save_world"));
  return (vi.mocked(invoke).mock.calls.at(-1)![1] as {constructionPlans:ConstructionPlans}).constructionPlans;
 };
 fireEvent.click(map,{clientX:102,clientY:200,detail:1});
 let saved=await save();
 expect(saved.draftPoints).toEqual([...anchors].reverse());
 expect(saved.draftEditedGeometry?.line).toEqual([...curve].reverse());
 expect(saved.plannedStations).toEqual([station]);
 fireEvent.click(map,{clientX:100,clientY:200,detail:1});
 fireEvent.keyDown(document.body,{key:"z",ctrlKey:true});
 saved=await save();
 expect(saved.draftPoints).toEqual(anchors);
 expect(saved.draftEditedGeometry).toBeUndefined();
 fireEvent.keyDown(document.body,{key:"Z",ctrlKey:true,shiftKey:true});
 saved=await save();expect(saved.draftEditedGeometry?.line).toEqual([...curve].reverse());
});
test("reversing a free start transfers the far built connection to source; occupied start cannot resume",async()=>{
 const destination={alignmentId:"3",endpoint:"start" as const};
 const anchors=[point(100,130),point(180,130)];
 const map=await start({construction_plans:{...emptyConstructionPlans(),draftPoints:anchors,draftFinished:true,draftDestination:destination}});
 fireEvent.click(map,{clientX:102,clientY:130,detail:1});
 vi.mocked(invoke).mockResolvedValueOnce({saved:true});fireEvent.click(screen.getByRole("button",{name:"保存"}));
 await waitFor(()=>expect(vi.mocked(invoke).mock.calls.at(-1)?.[0]).toBe("save_world"));
 const saved=(vi.mocked(invoke).mock.calls.at(-1)![1] as {constructionPlans:ConstructionPlans}).constructionPlans;
 expect(saved.draftExtension).toEqual(destination);expect(saved.draftDestination).toBeUndefined();
 expect(saved.draftPoints).toEqual([...anchors].reverse());
 fireEvent.click(map,{clientX:100,clientY:130,detail:1});
 fireEvent.click(map,{clientX:180,clientY:130,detail:1});
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
 expect(map.querySelectorAll(".parked-plan")).toHaveLength(0);
});
