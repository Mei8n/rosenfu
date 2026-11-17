import {settledMapClick} from './testMapClick';
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
 vi.mocked(isTauri).mockReturnValue(true);vi.mocked(invoke).mockImplementation(async command=>{if(command==="preview_construction_plan")return {validationError:null};
 if(command==="engine_info")return {name:"Rosenfu",version:"extension-test"};if(command==="world_snapshot")return {...world,...extra};if(command==="list_save_slots")return {active_slot_id:"test-slot",slots:[{id:"test-slot",name:"試験",loadable:true}]};if(command==="autosave_status")return {available:false};throw Error("unexpected "+command);
 });
 // Resolve the mocked startup IPC and restored-plan effects before the first key.
 // Version text alone can render before the window keyboard listener is refreshed.
 await act(async()=>{render(<App/>);});
 fireEvent.click(screen.getByRole("button",{name:"自作サンプル（オフライン）"}));expect(screen.getByText(/extension-test/)).toBeTruthy();
 const map=screen.getByRole("img",{name:"駅配置用サンプル地図"});map.getBoundingClientRect=()=>rect;return map;
}



test.each([false,true])('finished two-anchor segment disappears in one Undo and returns in one Redo (extension=%s)',async extension=>{
 const map=await start();const click=async (x:number,y=200)=>await settledMapClick(map,{clientX:x,clientY:y,detail:1});
 if(extension){await click(250,130);await click(350,130);await click(350,130);}else{await click(120);await click(240);await click(240);}
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
});
test('completed three-anchor path undoes C first then the last AB segment, redoing both in order',async()=>{
 const map=await start({alignments:[]});const click=async (x:number)=>await settledMapClick(map,{clientX:x,clientY:200,detail:1});
 await click(100);await click(200);await click(300);await click(300);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(3);
});

test('loaded completed AB has no session Undo or Redo history',async()=>{
 const map=await start({alignments:[],construction_plans:{...emptyConstructionPlans(),draftPoints:[point(120,200),point(240,200)],draftFinished:true}});
 const before=map.querySelector('.draft-path')!.getAttribute('d');
 for(const key of [{key:'z',ctrlKey:true},{key:'Z',ctrlKey:true,shiftKey:true}]){
  fireEvent.keyDown(document.body,key);expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
  expect(map.querySelector('.draft-path')!.getAttribute('d')).toBe(before);
 }
});

test('atomic segment history survives switching to an existing plan',async()=>{
 const map=await start({alignments:[]});const click=async (x:number)=>await settledMapClick(map,{clientX:x,clientY:200,detail:1});
 await click(120);await click(240);await click(240);
 fireEvent.click(screen.getByRole('button',{name:'別の線路計画を追加'}));
 fireEvent.click(screen.getByRole('button',{name:/計画 1 を編集/}));
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
});
test('station placement and its line undo chronologically without orphan dependencies',async()=>{
 const map=await start({alignments:[]});const click=async (x:number,y=200)=>await settledMapClick(map,{clientX:x,clientY:y,detail:1});
 await click(120);await click(240);await click(240);
 fireEvent.click(screen.getByRole('tab',{name:'駅設置'}));await click(180);fireEvent.click(screen.getByRole('tab',{name:'線路'}));
 expect(map.querySelectorAll('.planned-station')).toHaveLength(1);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);expect(map.querySelectorAll('.planned-station')).toHaveLength(0);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(map.querySelectorAll('.planned-station')).toHaveLength(1);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 await click(120,80);await click(240,80);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});
 expect(map.querySelectorAll('.planned-station')).toHaveLength(0);
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
});

test('loaded ABC remains intact across repeated Undo and Redo keys',async()=>{
 const map=await start({alignments:[],construction_plans:{...emptyConstructionPlans(),draftPoints:[point(100,200),point(200,200),point(300,200)],draftFinished:true}});
 const before=map.querySelector('.draft-path')!.getAttribute('d');
 for(let cycle=0;cycle<2;cycle++){
  fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});
  expect(map.querySelectorAll('.draft-anchor')).toHaveLength(3);expect(map.querySelector('.draft-path')!.getAttribute('d')).toBe(before);
 }
});

test('legacy saved redoPoints recover visible plans without becoming session history',async()=>{
 const map=await start({alignments:[],construction_plans:{...emptyConstructionPlans(),draftPoints:[],redoPoints:[point(300,200),point(200,200),point(100,200)],draftFinished:true}});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(3);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(3);
});

test('saving preserves session Redo but loading clears it and preserves saved geometry',async()=>{
 const map=await start({alignments:[]});
 for(const x of [100,300,300])await settledMapClick(map,{clientX:x,clientY:200,detail:1});
 const before=map.querySelector('.draft-path:not(.parked-plan)')!.getAttribute('d');
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);
 vi.mocked(invoke).mockResolvedValueOnce({saved:true});
 fireEvent.click(screen.getByRole('button',{name:'保存'}));await screen.findByText(/ワールドと未着工計画を保存しました/);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});
 expect(map.querySelector('.draft-path:not(.parked-plan)')!.getAttribute('d')).toBe(before);
 const anchors=[point(100,200),point(300,200)],geometry={version:1,anchors,line:[anchors[0],point(200,190),anchors[1]]};
 vi.mocked(invoke).mockResolvedValueOnce({...world,alignments:[],construction_plans:{...emptyConstructionPlans(),draftPoints:anchors,draftEditedGeometry:geometry,draftFinished:true}});
 await act(async()=>{fireEvent.click(screen.getByRole('button',{name:'読込'}));});
 const confirm=screen.queryByRole('button',{name:'読込を続ける'});if(confirm)await act(async()=>{fireEvent.click(confirm);});
 expect(screen.getByText(/保存したワールドを読み込みました/)).toBeTruthy();
 const loaded=map.querySelector('.draft-path:not(.parked-plan)')!.getAttribute('d');expect(loaded).not.toBe(before);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);expect(map.querySelector('.draft-path:not(.parked-plan)')!.getAttribute('d')).toBe(loaded);
});

test('fully undone second path does not block construction of the first path',async()=>{
 const map=await start({alignments:[]});const click=async (x:number,y:number)=>await settledMapClick(map,{clientX:x,clientY:y,detail:1});
 await click(100,100);await click(300,100);await click(300,100);
 await click(100,220);await click(300,220);await click(300,220);
 expect(map.querySelectorAll('.parked-anchor')).toHaveLength(2);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 expect(map.querySelectorAll('.draft-anchor,.parked-anchor')).toHaveLength(2);
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 vi.mocked(invoke).mockResolvedValueOnce({...world,alignments:[]});
 await act(async()=>{fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));});
 const call=vi.mocked(invoke).mock.calls.find(([cmd])=>cmd==='build_alignment');
 expect(call).toBeTruthy();expect((call![1] as any).points).toEqual([point(100,100),point(300,100)]);
 expect(map.querySelectorAll('.draft-anchor,.parked-anchor')).toHaveLength(0);
});

test('loaded parked redo-only paths recover as unbuilt candidates beside the active path',async()=>{
 const a=[point(100,100),point(300,100)],b=[point(100,220),point(300,220)];
 await start({alignments:[],construction_plans:{...emptyConstructionPlans(),draftPoints:a,draftFinished:true,parkedPaths:[{id:2,planId:1,name:'',points:[],redo:[...b].reverse(),line:[],trackType:'normal',designSpeedKmh:120,roadFollowing:false,extension:null}]}});
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
});

test('unfinished one-anchor path still blocks construction of another path',async()=>{
 const map=await start({alignments:[]});const click=async (x:number,y:number)=>await settledMapClick(map,{clientX:x,clientY:y,detail:1});
 await click(100,100);await click(300,100);await click(300,100);await click(100,220);
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(true);
});

test('Undo removes the later station before its path, without an orphan blocking construction',async()=>{
 const map=await start({alignments:[]});const click=async (x:number,y:number)=>await settledMapClick(map,{clientX:x,clientY:y,detail:1});
 await click(100,100);await click(300,100);await click(300,100);await click(100,220);await click(300,220);await click(300,220);
 fireEvent.click(screen.getByRole('tab',{name:'駅設置'}));await click(200,220);fireEvent.click(screen.getByRole('tab',{name:'線路'}));
 expect(map.querySelectorAll('.planned-station')).toHaveLength(1);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelectorAll('.planned-station')).toHaveLength(0);
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 expect(map.querySelectorAll('.draft-anchor,.parked-anchor')).toHaveLength(2);
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});
 expect(map.querySelectorAll('.planned-station')).toHaveLength(1);
});

test('Undo crosses independent paths within one plan and Redo restores each in order',async()=>{
 const map=await start({alignments:[]});
 const click=async (x:number,y:number)=>await settledMapClick(map,{clientX:x,clientY:y,detail:1});
 const shapes:string[]=[];
 for(const y of [80,150,220]){await click(100,y);await click(300,y);await click(300,y);shapes.push(map.querySelector('.draft-path:not(.parked-plan)')!.getAttribute('d')!);}
 const visibleShapes=()=>Array.from(map.querySelectorAll('.draft-path')).map(path=>path.getAttribute('d')).filter(Boolean).sort();
 const anchors=()=>map.querySelectorAll('.draft-anchor, .parked-anchor').length;
 expect(anchors()).toBe(6);
 for(const count of [4,2,0]){
  fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
  expect(anchors()).toBe(count);
  expect(visibleShapes()).toEqual(shapes.slice(0,count/2).sort());
 }
 for(const count of [2,4,6]){
  fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});
  expect(anchors()).toBe(count);
  expect(visibleShapes()).toEqual(shapes.slice(0,count/2).sort());
  expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
 }
 for(const count of [4,2,0]){
  fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
  expect(anchors()).toBe(count);
  expect(visibleShapes()).toEqual(shapes.slice(0,count/2).sort());
 }
});

test('Undo crosses construction plans in global chronological order',async()=>{
 const map=await start({alignments:[]});
 const click=async (x:number,y:number)=>await settledMapClick(map,{clientX:x,clientY:y,detail:1});
 await click(100,100);await click(300,100);await click(300,100);
 fireEvent.click(screen.getByRole('button',{name:'別の線路計画を追加'}));
 await click(100,220);await click(300,220);await click(300,220);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 expect(map.querySelectorAll('.draft-anchor, .parked-anchor')).toHaveLength(0);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});
 expect(map.querySelectorAll('.draft-anchor, .parked-anchor')).toHaveLength(2);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});
 expect(map.querySelectorAll('.draft-anchor, .parked-anchor')).toHaveLength(4);
});
