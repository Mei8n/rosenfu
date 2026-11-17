import {settledMapClick} from './testMapClick';
// @vitest-environment jsdom
import React from "react";
import {afterEach,expect,test,vi} from "vitest";
import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {invoke,isTauri} from "@tauri-apps/api/core";
import {App} from "./App";
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
 if(command==="engine_info")return {name:"Rosenfu",version:"extension-test"};if(command==="world_snapshot")return {...world,...extra};throw Error("unexpected "+command);
 });render(<App/>);fireEvent.click(screen.getByRole("button",{name:"自作サンプル（オフライン）"}));await screen.findByText(/extension-test/);
 const map=screen.getByRole("img",{name:"駅配置用サンプル地図"});map.getBoundingClientRect=()=>rect;return map;
}


test.each([false,true])('Undo follows edits across plan selection rather than the selected plan (extension=%s)',async extension=>{
 const map=await start();
 const click=async (x:number,y=200)=>await settledMapClick(map,{clientX:x,clientY:y,detail:1});
 if(extension){await click(250,130);await click(350,130);await click(350,130);}else{await click(120);await click(240);await click(240);}
 const firstShape=map.querySelector('.draft-path:not(.parked-plan)')!.getAttribute('d');
 fireEvent.click(screen.getByRole('button',{name:'別の線路計画を追加'}));
 await click(100,60);await click(200,60);await click(200,60);
 fireEvent.click(screen.getByRole('button',{name:/計画 1 を編集/}));
 const count=()=>map.querySelectorAll('.draft-anchor,.parked-anchor').length;
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(count()).toBe(2);
 expect(Array.from(map.querySelectorAll('.draft-path')).some(path=>path.getAttribute('d')===firstShape)).toBe(true);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(count()).toBe(0);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(count()).toBe(2);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(count()).toBe(4);
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
});

test('switching preserves drag geometry Redo and does not automatically resume drawing',async()=>{
 const map=await start({alignments:[]});
 for(const x of [150,250,350])await settledMapClick(map,{clientX:x,clientY:150});
 const shape=()=>map.querySelector('.draft-path:not(.parked-plan)')!.getAttribute('d');
 const before=shape(),anchor=map.querySelector('[data-anchor-index="1"]')!;
 fireEvent.pointerDown(anchor,{clientX:250,clientY:150,button:0});
 fireEvent.pointerMove(map,{clientX:250,clientY:160});fireEvent.pointerUp(map,{clientX:250,clientY:160});
 const moved=shape();expect(moved).not.toBe(before);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(shape()).toBe(before);
 fireEvent.click(screen.getByRole('button',{name:'別の線路計画を追加'}));
 fireEvent.click(screen.getByRole('button',{name:/計画 1 を編集/}));
 fireEvent.pointerMove(map,{clientX:400,clientY:180});expect(shape()).toBe(before);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(shape()).toBe(moved);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(shape()).toBe(before);
});

test('repeated plan navigation keeps one chronological history and new editing discards global Redo',async()=>{
 const map=await start({alignments:[]});
 const click=async (x:number,y:number)=>await settledMapClick(map,{clientX:x,clientY:y,detail:1});
 const undo=()=>fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 const redo=()=>fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});
 const count=()=>map.querySelectorAll('.draft-anchor,.parked-anchor').length;
 await click(120,200);await click(240,200);await click(240,200);
 fireEvent.click(screen.getByRole('button',{name:'別の線路計画を追加'}));
 await click(120,80);await click(240,80);await click(240,80);
 for(let i=0;i<2;i++){
  fireEvent.click(screen.getByRole('button',{name:/計画 1 を編集/}));
  fireEvent.click(screen.getByRole('button',{name:/計画 2 を編集/}));
 }
 undo();expect(count()).toBe(2);undo();expect(count()).toBe(0);
 redo();expect(count()).toBe(2);redo();expect(count()).toBe(4);
 undo();expect(count()).toBe(2);
 await click(120,60);await click(240,60);await click(240,60);
 const shape=Array.from(map.querySelectorAll('.draft-path')).map(path=>path.getAttribute('d'));
 redo();expect(count()).toBe(4);
 expect(Array.from(map.querySelectorAll('.draft-path')).map(path=>path.getAttribute('d'))).toEqual(shape);
});
