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
 vi.mocked(isTauri).mockReturnValue(true);vi.mocked(invoke).mockImplementation(async command=>{
 if(command==="engine_info")return {name:"Rosenfu",version:"extension-test"};if(command==="world_snapshot")return {...world,...extra};throw Error("unexpected "+command);
 });render(<App/>);fireEvent.click(screen.getByRole("button",{name:"自作サンプル（オフライン）"}));await screen.findByText(/extension-test/);
 const map=screen.getByRole("img",{name:"駅配置用サンプル地図"});map.getBoundingClientRect=()=>rect;return map;
}


test.each(['駅設置','削除','Escape'])('unfinished path survives %s and reentry with construction enabled',async tool=>{
 const map=await start({alignments:[]});
 const click=(x:number,y=200)=>fireEvent.click(map,{clientX:x,clientY:y,detail:1});
 click(120);click(240);
 if(tool==='Escape')fireEvent.keyDown(document.body,{key:'Escape'});
 else fireEvent.click(screen.getByRole('tab',{name:tool}));
 fireEvent.click(screen.getByRole('tab',{name:'線路'}));
 click(300);
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
});
test.each(['駅設置','削除','Escape'])('finished path resumes explicitly after %s with construction enabled',async tool=>{
 const map=await start({alignments:[]});
 const click=(x:number,y=200)=>fireEvent.click(map,{clientX:x,clientY:y,detail:1});
 click(120);click(240);click(240);
 if(tool==='Escape')fireEvent.keyDown(document.body,{key:'Escape'});
 else fireEvent.click(screen.getByRole('tab',{name:tool}));
 fireEvent.click(screen.getByRole('tab',{name:'線路'}));
 click(240);click(300);
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
});
test.each(['駅設置','削除','Escape'])('building then returning via %s allows the next plan',async tool=>{
 const map=await start({alignments:[]});
 const click=(x:number,y=200)=>fireEvent.click(map,{clientX:x,clientY:y,detail:1});
 click(120);click(240);
 vi.mocked(invoke).mockResolvedValueOnce({...world,alignments:[]});
 fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));
 await screen.findByText(/線路を建設しました/);
 if(tool==='Escape')fireEvent.keyDown(document.body,{key:'Escape'});
 else fireEvent.click(screen.getByRole('tab',{name:tool}));
 fireEvent.click(screen.getByRole('tab',{name:'線路'}));
 click(120,80);click(240,80);
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
});
test.each([false,true])('all anchors undone then new drawing builds without a hidden redo-only path (extension=%s)',async extension=>{
 const map=await start();
 const click=(x:number,y=200)=>fireEvent.click(map,{clientX:x,clientY:y,detail:1});
 click(120);click(240);click(240);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);
 if(extension){click(250,130);click(350,130);}
 else {click(120,80);click(240,80);}
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
});
test('switching plans retains a fully undone start point for Redo',async()=>{
 const map=await start({alignments:[]});
 fireEvent.click(map,{clientX:120,clientY:200,detail:1});
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 fireEvent.click(screen.getByRole('button',{name:'別の線路計画を追加'}));
 fireEvent.click(screen.getByRole('button',{name:/計画 1 を編集/}));
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(1);
});
