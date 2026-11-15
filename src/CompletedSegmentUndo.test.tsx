// @vitest-environment jsdom
import React from "react";
import {afterEach,expect,test,vi} from "vitest";
import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
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
 });
 // Resolve the mocked startup IPC and restored-plan effects before the first key.
 // Version text alone can render before the window keyboard listener is refreshed.
 await act(async()=>{render(<App/>);});
 fireEvent.click(screen.getByRole("button",{name:"自作サンプル（オフライン）"}));expect(screen.getByText(/extension-test/)).toBeTruthy();
 const map=screen.getByRole("img",{name:"駅配置用サンプル地図"});map.getBoundingClientRect=()=>rect;return map;
}



test.each([false,true])('finished two-anchor segment disappears in one Undo and returns in one Redo (extension=%s)',async extension=>{
 const map=await start();const click=(x:number,y=200)=>fireEvent.click(map,{clientX:x,clientY:y,detail:1});
 if(extension){click(250,130);click(350,130);click(350,130);}else{click(120);click(240);click(240);}
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
});
test('completed three-anchor path undoes C first then the last AB segment, redoing both in order',async()=>{
 const map=await start({alignments:[]});const click=(x:number)=>fireEvent.click(map,{clientX:x,clientY:200,detail:1});
 click(100);click(200);click(300);click(300);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(3);
});

test('loaded completed AB has the same atomic last-segment Undo and Redo',async()=>{
 const map=await start({alignments:[],construction_plans:{...emptyConstructionPlans(),draftPoints:[point(120,200),point(240,200)],draftFinished:true}});
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
});
test('atomic segment history survives switching to an existing plan',async()=>{
 const map=await start({alignments:[]});const click=(x:number)=>fireEvent.click(map,{clientX:x,clientY:200,detail:1});
 click(120);click(240);click(240);
 fireEvent.click(screen.getByRole('button',{name:'別の線路計画を追加'}));
 fireEvent.click(screen.getByRole('button',{name:/計画 1 を編集/}));
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);
 fireEvent.click(screen.getByRole('button',{name:/計画 2 を編集/}));
 fireEvent.click(screen.getByRole('button',{name:/計画 1 を編集/}));
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
});
test('station draft stays with Undo for Redo and a replacement drawing discards only old dependencies',async()=>{
 const map=await start({alignments:[]});const click=(x:number,y=200)=>fireEvent.click(map,{clientX:x,clientY:y,detail:1});
 click(120);click(240);click(240);
 fireEvent.click(screen.getByRole('tab',{name:'駅設置'}));click(180);fireEvent.click(screen.getByRole('tab',{name:'線路'}));
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);
 expect(map.querySelectorAll('.planned-station')).toHaveLength(1);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 expect(map.querySelectorAll('.planned-station')).toHaveLength(1);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 click(120,80);click(240,80);
 expect(map.querySelectorAll('.planned-station')).toHaveLength(0);
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
});

test('loaded ABC supports repeated undo/redo cycles across the final segment boundary',async()=>{
 const map=await start({alignments:[],construction_plans:{...emptyConstructionPlans(),draftPoints:[point(100,200),point(200,200),point(300,200)],draftFinished:true}});
 for(let cycle=0;cycle<2;cycle++){
  fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
  fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);
  fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
  fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(3);
 }
});

test('saved fully undone completed path restores AB together without session history',async()=>{
 const map=await start({alignments:[],construction_plans:{...emptyConstructionPlans(),draftPoints:[],redoPoints:[point(300,200),point(200,200),point(100,200)],draftFinished:true}});
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(3);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);
});

test('atomic Undo saves explicit sampled geometry for exact Redo after reload',async()=>{
 const anchors=[point(100,200),point(300,200)],geometry={version:1,anchors,line:[anchors[0],point(200,190),anchors[1]]};
 const map=await start({alignments:[],construction_plans:{...emptyConstructionPlans(),draftPoints:anchors,draftEditedGeometry:geometry,draftFinished:true}});
 const before=map.querySelector('.draft-path:not(.parked-plan)')!.getAttribute('d');
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);
 vi.mocked(invoke).mockResolvedValueOnce({saved:true});
 fireEvent.click(screen.getByRole('button',{name:'保存'}));await screen.findByText(/ワールドと未着工計画を保存しました/);
 const payload=vi.mocked(invoke).mock.calls.find(([cmd])=>cmd==='save_world')![1] as any;
 expect(payload.constructionPlans.draftEditedGeometry).toEqual(geometry);
 vi.mocked(invoke).mockResolvedValueOnce({...world,alignments:[],construction_plans:payload.constructionPlans});
 await act(async()=>{fireEvent.click(screen.getByRole('button',{name:'読込'}));});
 expect(screen.getByText(/保存したワールドを読み込みました/)).toBeTruthy();
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 expect(map.querySelector('.draft-path:not(.parked-plan)')!.getAttribute('d')).toBe(before);
});
