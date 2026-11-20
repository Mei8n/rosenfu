// @vitest-environment jsdom
import React from "react";
import {afterEach,expect,test,vi} from "vitest";
import {act,cleanup,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {invoke,isTauri} from "@tauri-apps/api/core";
import {App} from "./App";
import {DISPLAY_SETTINGS_KEY} from "./displaySettings";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();localStorage.removeItem(DISPLAY_SETTINGS_KEY);vi.resetAllMocks();});
const initial={next_id:"20",players:[{id:"1",name:"Player"}],companies:[{id:"2",owner:"1",name:"Railway"}],stations:[{id:"10",owner:"2",name:"既存駅",latitude:35.67504,longitude:139.73}],routes:[] as any[],route_paths:[] as any[],services:[],station_placements:[{station_id:"10",alignment_id:"3",position:1/6,length_m:200,width_m:12}] as any[],alignments:[{id:"3",owner:"2",name:"試験線",points:[{latitude:35.67504,longitude:139.72},{latitude:35.67504,longitude:139.78}]}]};
// Inactive editors remain mounted: scope semantic queries to the owning UI region.
function region(selector:string){
 const element=document.querySelector<HTMLElement>(selector);
 if(!element)throw new Error(`Missing test region: ${selector}`);
 return within(element);
}
async function start(){let world=structuredClone(initial);let id=20;
 const undoStack:typeof initial[]=[],redoStack:typeof initial[]=[];
 const failures={remove:false,place:false,save:false,load:false,undo:false,redo:false};
 const commit=(next:typeof initial,final=false)=>{if(final){undoStack.length=0;redoStack.length=0;}else{undoStack.push(structuredClone(world));redoStack.length=0;}world=next;return structuredClone(world);};
 vi.mocked(isTauri).mockReturnValue(true);vi.mocked(invoke).mockImplementation(async(command,args)=>{
 const p=args as any;
 if(command==="autosave_status")return {available:false};if(command==="list_save_slots")return {active_slot_id:"test-slot",slots:[]};if(command==="engine_info")return {name:"Rosenfu",version:"history-test"};if(command==="world_snapshot")return structuredClone(world);
 if(command==="preview_construction_plan")return {validationError:null};
 if(command==="place_station_on_alignment"||command==="build_plan") {if(failures.place)throw Error("place failure");const stationPlan=command==="build_plan"?p.stations[0]:p;const station={id:String(id++),owner:"2",name:stationPlan.name,latitude:35.67504,longitude:139.75};return commit({...world,stations:[...world.stations,station],station_placements:[...world.station_placements,{station_id:station.id,alignment_id:stationPlan.alignmentId,position:stationPlan.position,length_m:stationPlan.lengthM,width_m:stationPlan.widthM}]},true);}
 if(command==="delete_station"){if(failures.remove)throw Error("delete failure");return commit({...world,stations:world.stations.filter(s=>s.id!==p.stationId),station_placements:world.station_placements.filter(s=>s.station_id!==p.stationId)});}
 if(command==="save_world"){if(failures.save)throw Error("save failure");return {saved:true};}
 if(command==="rename_station")return commit({...world,stations:world.stations.map(station=>station.id===p.stationId?{...station,name:p.name}:station)});
 if(command==="update_station_placement")return commit({...world,station_placements:world.station_placements.map(placement=>placement.station_id===p.stationId?{...placement,length_m:p.lengthM,width_m:p.widthM}:placement)},true);
 if(command==="preview_route_candidates")return {status:"unique",truncated:false,candidates:[{stations:["10","20"],segments:[{kind:"alignment",alignment_id:"3",from:"10",to:"20"}]}]};
 if(command==="preview_route_membership")return {path:{stations:["10","20"],segments:p.segments},member_stations:["10","20"],excluded_stations:[]};
 if(command==="save_planned_route")return commit({...world,routes:[{id:"30",owner:"2",name:p.name,start:p.start}],route_paths:[{route_id:"30",segments:p.segments,member_stations:["10","20"]}]});
 if(command==="undo_world_edit"||command==="redo_world_edit"){
  const redo=command==='redo_world_edit';if(failures[redo?'redo':'undo'])throw Error(redo?'redo failure':'undo failure');
  const source=redo?redoStack:undoStack,target=source.pop();if(!target)throw Error('missing history');
  (redo?undoStack:redoStack).push(structuredClone(world));world=target;
  return {...structuredClone(world),construction_plans:p.expectedWorld.construction_plans,view_state:p.expectedWorld.view_state};
 }
 if(command==="load_save_slot"){if(failures.load)throw Error("load failure");world=structuredClone(initial);undoStack.length=0;redoStack.length=0;return world;}
 throw Error(command);
 });render(<App/>);await screen.findByText(/history-test/);
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith("autosave_status",{slotId:"test-slot"}));
 await act(async()=>{});await ready();fireEvent.click(screen.getByRole("tab",{name:"建設"}));fireEvent.click(region(".editor-tabs").getByRole("tab",{name:"駅設置"}));return failures;}
async function place(x=250){const map=screen.getByRole("img",{name:"駅配置用OpenStreetMap"});map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});fireEvent.click(map,{clientX:x,clientY:150});await screen.findByText(/を計画に追加しました/);fireEvent.click(screen.getByRole("button",{name:"計画を着工"}));await screen.findByText(/計画駅を建設しました/);await ready();}
async function ready(){await waitFor(()=>expect((document.querySelector(".save-controls button:last-child") as HTMLButtonElement).disabled).toBe(false));}
const undo=()=>fireEvent.keyDown(document.body,{key:"z",ctrlKey:true});const redo=()=>fireEvent.keyDown(document.body,{key:"z",ctrlKey:true,shiftKey:true});

async function selectStation(name='既存駅'){fireEvent.click(screen.getByRole('tab',{name:'駅'}));fireEvent.click(region('.station-directory').getByRole('button',{name:new RegExp('^'+name+' ')}));}
async function rename(name='改名駅'){await selectStation();fireEvent.change(screen.getByLabelText('選択駅の名前'),{target:{value:name}});fireEvent.click(screen.getByRole('button',{name:'駅名を適用'}));await screen.findByText(/駅名を変更しました/);await ready();}
async function step(redoStep=false){await act(async()=>{if(redoStep)redo();else undo();});await ready();}
const calls=(command:string)=>vi.mocked(invoke).mock.calls.filter(([name])=>name===command);
function stationName(name:string){return region('.station-directory').queryByRole('button',{name:new RegExp('^'+name+' '),hidden:true});}

test('construction ends history and cannot remove an existing or newly built station',async()=>{
 await start();await rename();fireEvent.click(screen.getByRole('tab',{name:'建設'}));fireEvent.click(region('.editor-tabs').getByRole('tab',{name:'駅設置'}));await place();await step();
 expect(calls('undo_world_edit')).toHaveLength(0);expect(calls('delete_station')).toHaveLength(0);expect(document.querySelectorAll('.station-marker')).toHaveLength(2);expect(stationName('改名駅')).toBeTruthy();
});
test('station rename Undo/Redo preserves stable ID and failures preserve both stacks',async()=>{
 const failures=await start();await rename();failures.undo=true;await step();expect(await screen.findByText(/undo failure/)).toBeTruthy();expect(stationName('改名駅')).toBeTruthy();
 failures.undo=false;await step();expect(stationName('既存駅')).toBeTruthy();failures.redo=true;await step(true);expect(await screen.findByText(/redo failure/)).toBeTruthy();expect(stationName('既存駅')).toBeTruthy();
 failures.redo=false;await step(true);expect(stationName('改名駅')).toBeTruthy();
 expect((calls('redo_world_edit').at(-1)![1] as any).expectedWorld.stations[0].id).toBe('10');expect(calls('delete_station')).toHaveLength(0);
});
test.each([false,true])('save retains history whether successful or failed (failed=%s)',async failed=>{
 const failures=await start();await rename();failures.save=failed;fireEvent.click(region('.save-controls').getByRole('button',{name:'保存'}));await screen.findByText(failed?/save failure/:'ワールドと未着工計画を保存しました。');await ready();await step();expect(stationName('既存駅')).toBeTruthy();await step(true);expect(stationName('改名駅')).toBeTruthy();
});
test.each([false,true])('load clears history only on success (failed=%s)',async failed=>{
 const failures=await start();await rename();failures.load=failed;fireEvent.click(region('.save-controls').getByRole('button',{name:'読込'}));fireEvent.click(region('.load-confirm').getByRole('button',{name:'読込を続ける'}));await screen.findByText(failed?/load failure/:'保存したワールドを読み込みました。');await ready();await step();expect(stationName('既存駅')).toBeTruthy();expect(calls('undo_world_edit')).toHaveLength(failed?1:0);
});
test('native text undo does not issue a world transaction',async()=>{
 await start();await rename();await selectStation('改名駅');fireEvent.keyDown(screen.getByLabelText('選択駅の名前'),{key:'z',ctrlKey:true});expect(calls('undo_world_edit')).toHaveLength(0);expect(stationName('改名駅')).toBeTruthy();
});
test('repeated keydown cannot issue duplicate undo while transaction is pending',async()=>{
 await start();await rename();const implementation=vi.mocked(invoke).getMockImplementation()!;let finish!:(value:unknown)=>void;
 vi.mocked(invoke).mockImplementation((command,args)=>command==='undo_world_edit'?new Promise(resolve=>{finish=resolve;}):implementation(command,args));
 undo();undo();fireEvent.keyDown(document.body,{key:'z',ctrlKey:true,repeat:true});expect(calls('undo_world_edit')).toHaveLength(1);await act(async()=>finish(initial));await ready();expect(stationName('既存駅')).toBeTruthy();
});
test('built station resizing commits previous rename history',async()=>{
 await start();await rename();await selectStation('改名駅');fireEvent.change(screen.getByLabelText('選択駅の長さ'),{target:{value:'12'}});await screen.findByText('ホーム寸法を変更しました。保存でファイルに反映します。');await ready();await step();expect(calls('undo_world_edit')).toHaveLength(0);expect(stationName('改名駅')).toBeTruthy();
});
test('route edit Undo removes its reference before station demolition is allowed',async()=>{
 await start();await place();fireEvent.click(screen.getByRole('tab',{name:'路線'}));fireEvent.click(screen.getByRole('button',{name:'新規路線'}));const markers=document.querySelectorAll('.station-marker');fireEvent.click(markers[0]);fireEvent.click(markers[1]);await waitFor(()=>expect((screen.getByRole('button',{name:'編集終了'}) as HTMLButtonElement).disabled).toBe(false));fireEvent.change(screen.getByLabelText('路線名'),{target:{value:'参照路線'}});fireEvent.click(screen.getByRole('button',{name:'編集終了'}));await screen.findByText(/路線と所属駅を適用しました/);await ready();
 await selectStation();expect((screen.getByRole('button',{name:'選択駅を削除'}) as HTMLButtonElement).disabled).toBe(true);await step();await selectStation();expect((screen.getByRole('button',{name:'選択駅を削除'}) as HTMLButtonElement).disabled).toBe(false);expect(document.querySelectorAll('.station-marker')).toHaveLength(2);expect(calls('delete_station')).toHaveLength(0);
});

test('demolition Undo/Redo keeps the station ID and failed demolition leaves prior rename undoable',async()=>{
 const failures=await start();await rename();await selectStation('改名駅');failures.remove=true;
 fireEvent.click(screen.getByRole('button',{name:'選択駅を削除'}));fireEvent.click(screen.getByRole('button',{name:'削除を確定'}));await screen.findByText(/delete failure/);await ready();fireEvent.click(screen.getByRole('button',{name:'削除をやめる'}));await step();expect(stationName('既存駅')).toBeTruthy();await step(true);
 await selectStation('改名駅');failures.remove=false;fireEvent.click(screen.getByRole('button',{name:'選択駅を削除'}));fireEvent.click(screen.getByRole('button',{name:'削除を確定'}));await waitFor(()=>expect(document.querySelectorAll('.station-marker')).toHaveLength(0));await ready();
 await step();expect(stationName('改名駅')).toBeTruthy();await step(true);expect(stationName('改名駅')).toBeNull();await step();expect(stationName('改名駅')).toBeTruthy();
 expect((calls('redo_world_edit').at(-1)![1] as any).expectedWorld.stations[0].id).toBe('10');expect(calls('build_plan')).toHaveLength(0);
});
test('failed construction preserves the planned station and earlier rename history',async()=>{
 const failures=await start();await rename();fireEvent.click(screen.getByRole('tab',{name:'建設'}));fireEvent.click(region('.editor-tabs').getByRole('tab',{name:'駅設置'}));
 const map=screen.getByRole('img',{name:'駅配置用OpenStreetMap'});map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 fireEvent.click(map,{clientX:250,clientY:150});await screen.findByText(/を計画に追加しました/);failures.place=true;fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));await screen.findByText(/place failure/);await ready();expect(map.querySelectorAll('.planned-station')).toHaveLength(1);
 await step();expect(map.querySelectorAll('.planned-station')).toHaveLength(0);expect(stationName('改名駅')).toBeTruthy();await step();expect(stationName('既存駅')).toBeTruthy();expect(calls('undo_world_edit')).toHaveLength(1);
});
