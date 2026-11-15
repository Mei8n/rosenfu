// @vitest-environment jsdom
import React from "react";
import {afterEach,expect,test,vi} from "vitest";
import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {invoke,isTauri} from "@tauri-apps/api/core";
import {App} from "./App";
import {emptyConstructionPlans} from "./constructionPlans";
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
 });render(<App/>);fireEvent.click(screen.getByRole("button",{name:"自作サンプル（オフライン）"}));await screen.findByText(/extension-test/);
 const map=screen.getByRole("img",{name:"駅配置用サンプル地図"});map.getBoundingClientRect=()=>rect;return map;
}

test('legacy saved single anchor resumes the same exact anchor, not a nearby new path',async()=>{
 const map=await start({alignments:[],construction_plans:{...emptyConstructionPlans(),draftPoints:[point(120,200)],redoPoints:[point(240,200)],draftFinished:true}});
 const click=(x:number,y=200)=>fireEvent.click(map,{clientX:x,clientY:y,detail:1});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(1);
 fireEvent.pointerMove(map,{clientX:122,clientY:201});
 expect(map.querySelector('.endpoint-snap-ring')).not.toBeNull();
 click(122,201);
 expect(map.querySelectorAll('.parked-anchor')).toHaveLength(0);
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(1);
 click(300);
 vi.mocked(invoke).mockResolvedValueOnce({...world,alignments:[]});
 fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('build_alignment',expect.objectContaining({points:expect.any(Array)})));
 const request=vi.mocked(invoke).mock.calls.find(([c])=>c==='build_alignment')![1] as {points:ReturnType<typeof point>[]};
 expect(request.points).toHaveLength(2);
 expect(request.points[0]).toEqual(point(120,200));
});

test('a parked single anchor can resume without duplicating either path',async()=>{
 const map=await start({alignments:[],construction_plans:{...emptyConstructionPlans(),draftPoints:[point(120,200)],redoPoints:[point(240,200)],draftFinished:true}});
 const click=(x:number,y=200)=>fireEvent.click(map,{clientX:x,clientY:y,detail:1});
 // Start and finish another disconnected track in this same plan.
 click(300,80);click(400,80);click(400,80);
 expect(map.querySelectorAll('.parked-anchor')).toHaveLength(1);
 fireEvent.pointerMove(map,{clientX:122,clientY:201});
 expect(map.querySelector('.endpoint-snap-ring')).not.toBeNull();
 click(122,201);
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(1);
 expect(map.querySelectorAll('.parked-anchor')).toHaveLength(2);
 click(240);
 vi.mocked(invoke).mockResolvedValueOnce({...world,alignments:[]});
 fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('build_plan',expect.anything()));
 const request=vi.mocked(invoke).mock.calls.find(([c])=>c==='build_plan')![1] as {alignments:{points:ReturnType<typeof point>[]}[]};
 expect(request.alignments).toHaveLength(2);
 expect(request.alignments.find(path=>path.points[0].latitude===point(120,200).latitude)?.points[0]).toEqual(point(120,200));
});
