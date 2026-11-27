import {settledMapClick} from './testMapClick';
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
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();vi.unstubAllGlobals();});
async function start(snapshot:unknown=built){
 vi.stubGlobal('PointerEvent',MouseEvent);vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async command=>{if(command==="preview_construction_plan")return {validationError:null};
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

test.each(['background','Escape'])('clears explicit built highlight with %s without changing the world',async action=>{
 const map=await start();fireEvent.click(screen.getByRole('button',{name:'選択'}));
 fireEvent.click(map.querySelector('.alignment-hit')!);
 expect(screen.getByRole('button',{name:'選択線路を削除'})).toBeTruthy();
 if(action==='background'){fireEvent.pointerDown(map,{clientX:20,clientY:20,button:0});fireEvent.pointerUp(map,{clientX:20,clientY:20,button:0});fireEvent.click(map,{clientX:20,clientY:20});}else fireEvent.keyDown(document.body,{key:'Escape'});
 expect(screen.queryByRole('button',{name:'選択線路を削除'})).toBeNull();
 expect(map.querySelector('.alignment-hit')).not.toBeNull();
 fireEvent.click(map.querySelector('.alignment-hit')!);
 expect(screen.getByRole('button',{name:'選択線路を削除'})).toBeTruthy();
});
test('Escape in a text field or confirmation does not clear the selected track',async()=>{
 const map=await start();fireEvent.click(screen.getByRole('button',{name:'選択'}));fireEvent.click(map.querySelector('.alignment-hit')!);
 const input=document.createElement('input');document.body.append(input);fireEvent.keyDown(input,{key:'Escape'});input.remove();
 expect(screen.getByRole('button',{name:'選択線路を削除'})).toBeTruthy();
 fireEvent.click(screen.getByRole('button',{name:'選択線路を削除'}));fireEvent.keyDown(document.body,{key:'Escape'});
 expect(screen.getByRole('dialog')).toBeTruthy();
 fireEvent.click(screen.getByRole('button',{name:'キャンセル'}));expect(screen.getByRole('button',{name:'選択線路を削除'})).toBeTruthy();
});
test('background and Escape clear planned highlighting but keep its geometry',async()=>{
 const map=await start(empty);await settledMapClick(map,{clientX:180,clientY:130});await settledMapClick(map,{clientX:230,clientY:130});
 const before=map.querySelector('.confirmed-draft')!.getAttribute('d');
 fireEvent.click(screen.getByRole('button',{name:'選択'}));fireEvent.click(map.querySelector('.plan-hover-hit')!);
 fireEvent.click(map,{clientX:20,clientY:20});expect(screen.queryByRole('button',{name:'選択線路を削除'})).toBeNull();
 expect(map.querySelector('.confirmed-draft')!.getAttribute('d')).toBe(before);
 fireEvent.click(map.querySelector('.plan-hover-hit')!);fireEvent.keyDown(document.body,{key:'Escape'});
 expect(screen.queryByRole('button',{name:'選択線路を削除'})).toBeNull();expect(map.querySelector('.confirmed-draft')!.getAttribute('d')).toBe(before);
});

test('station selection clears a track while panning does not',async()=>{
 const map=await start({...built,stations:[{id:'4',owner:'2',name:'駅',latitude:35.67504,longitude:139.75}]});
 fireEvent.click(screen.getByRole('button',{name:'選択'}));fireEvent.click(map.querySelector('.alignment-hit')!);
 fireEvent.pointerDown(map,{clientX:20,clientY:20,button:0});fireEvent.pointerMove(map,{clientX:60,clientY:20,buttons:1});fireEvent.pointerUp(map,{clientX:60,clientY:20,button:0});fireEvent.click(map,{clientX:60,clientY:20});
 expect(screen.getByRole('button',{name:'選択線路を削除'})).toBeTruthy();
 fireEvent.pointerDown(map,{clientX:20,clientY:20,button:0});fireEvent.pointerUp(map,{clientX:20,clientY:20,button:0});
 fireEvent.click(map.querySelector('.station-marker')!);
 expect(screen.queryByRole('button',{name:'選択線路を削除'})).toBeNull();
 expect(map.querySelector('.alignment-hit')).not.toBeNull();
});
