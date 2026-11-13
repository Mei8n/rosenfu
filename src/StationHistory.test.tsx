// @vitest-environment jsdom
import React from "react";
import {afterEach,expect,test,vi} from "vitest";
import {cleanup,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {invoke,isTauri} from "@tauri-apps/api/core";
import {App} from "./App";
import {DISPLAY_SETTINGS_KEY} from "./displaySettings";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();localStorage.removeItem(DISPLAY_SETTINGS_KEY);vi.resetAllMocks();});
const initial={next_id:"20",players:[{id:"1",name:"Player"}],companies:[{id:"2",owner:"1",name:"Railway"}],stations:[{id:"10",owner:"2",name:"既存駅",latitude:35.67504,longitude:139.73}],tracks:[] as any[],routes:[] as any[],route_paths:[] as any[],services:[],station_placements:[{station_id:"10",alignment_id:"3",position:1/6,length_m:200,width_m:12}] as any[],alignments:[{id:"3",owner:"2",name:"試験線",points:[{latitude:35.67504,longitude:139.72},{latitude:35.67504,longitude:139.78}]}]};
// Inactive editors remain mounted: scope semantic queries to the owning UI region.
function region(selector:string){
 const element=document.querySelector<HTMLElement>(selector);
 if(!element)throw new Error(`Missing test region: ${selector}`);
 return within(element);
}
async function start(){let world=structuredClone(initial);let id=20;const failures={remove:false,place:false,save:false,load:false};vi.mocked(isTauri).mockReturnValue(true);vi.mocked(invoke).mockImplementation(async(command,args)=>{
 if(command==="engine_info")return {name:"Rosenfu",version:"history-test"};if(command==="world_snapshot")return world;
 if(command==="place_station_on_alignment") {if(failures.place)throw Error("place failure");const p=args as any;const station={id:String(id++),owner:"2",name:p.name,latitude:35.67504,longitude:139.75};world={...world,stations:[...world.stations,station],station_placements:[...world.station_placements,{station_id:station.id,alignment_id:p.alignmentId,position:p.position,length_m:p.lengthM,width_m:p.widthM}]};return world;}
 if(command==="delete_station"){if(failures.remove)throw Error("delete failure");const stationId=(args as any).stationId;world={...world,stations:world.stations.filter(s=>s.id!==stationId),station_placements:world.station_placements.filter(p=>p.station_id!==stationId)};return world;}
 if(command==="add_track"){world={...world,tracks:[{id:"99",owner:"2",...(args as any)}]};return world;}
 if(command==="save_world"){if(failures.save)throw Error("save failure");return {saved:true};}
 if(command==="rename_station"){const p=args as any;world={...world,stations:world.stations.map(station=>station.id===p.stationId?{...station,name:p.name}:station)};return world;}
 if(command==="update_station_placement"){const p=args as any;world={...world,station_placements:world.station_placements.map(placement=>placement.station_id===p.stationId?{...placement,length_m:p.lengthM,width_m:p.widthM}:placement)};return world;}
 if(command==="preview_route_candidates")return {status:"unique",truncated:false,candidates:[{stations:["10","20"],segments:[{kind:"alignment",alignment_id:"3",from:"10",to:"20"}],track_only:false}]};
 if(command==="preview_route_membership")return {path:{stations:["10","20"],segments:(args as any).segments,track_only:false},member_stations:["10","20"],excluded_stations:[]};
 if(command==="save_planned_route"){const p=args as any;world={...world,routes:[{id:"30",owner:"2",name:p.name,start:p.start,tracks:[]}],route_paths:[{route_id:"30",segments:p.segments,member_stations:["10","20"]}]};return world;}
 if(command==="load_world"){if(failures.load)throw Error("load failure");world=structuredClone(initial);return world;}
 throw Error(command);
 });render(<App/>);await screen.findByText(/history-test/);await ready();fireEvent.click(screen.getByRole("tab",{name:"建設"}));fireEvent.click(region(".editor-tabs").getByRole("tab",{name:"駅設置"}));return failures;}
async function place(){const map=screen.getByRole("img",{name:"駅配置用OpenStreetMap"});map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});fireEvent.click(map,{clientX:250,clientY:150});await screen.findByText(/を線路に沿って配置しました/);await ready();}
async function ready(){await waitFor(()=>expect((document.querySelector(".save-controls button:last-child") as HTMLButtonElement).disabled).toBe(false));}
const undo=()=>fireEvent.keyDown(document.body,{key:"z",ctrlKey:true});const redo=()=>fireEvent.keyDown(document.body,{key:"z",ctrlKey:true,shiftKey:true});
test("station undo and redo preserve failed history, issue a fresh ID and protect existing stations",async()=>{
 const failures=await start();await place();failures.remove=true;undo();await screen.findByText(/delete failure/);await ready();expect((region(".station-history").getByRole("button",{name:"駅設置を取り消す"}) as HTMLButtonElement).disabled).toBe(false);
 failures.remove=false;undo();await screen.findByText(/直前の駅設置を取り消しました/);await ready();expect(invoke).toHaveBeenCalledWith("delete_station",{stationId:"20"});expect(region(".station-directory").getByRole("button",{name:/^既存駅 /})).toBeTruthy();
 failures.place=true;redo();await screen.findByText(/place failure/);await ready();expect((region(".station-history").getByRole("button",{name:"駅設置をやり直す"}) as HTMLButtonElement).disabled).toBe(false);
 failures.place=false;redo();await screen.findByText(/駅を再設置しました/);await ready();undo();await screen.findByText(/直前の駅設置を取り消しました/);await ready();expect(invoke).toHaveBeenCalledWith("delete_station",{stationId:"21"});
});
test("successful save closes history while failed save preserves it",async()=>{
 const failures=await start();await place();failures.save=true;fireEvent.click(region(".save-controls").getByRole("button",{name:"保存"}));await screen.findByText(/save failure/);await ready();expect((region(".station-history").getByRole("button",{name:"駅設置を取り消す"}) as HTMLButtonElement).disabled).toBe(false);
 failures.save=false;fireEvent.click(region(".save-controls").getByRole("button",{name:"保存"}));await screen.findByText("ワールドを保存しました。");await ready();undo();await screen.findByText(/取り消せる駅設置履歴はありません/);await ready();expect(vi.mocked(invoke).mock.calls.some(([command])=>command==="delete_station")).toBe(false);
});
test("references block station undo without consuming history",async()=>{
 await start();await place();fireEvent.click(region(".editor-tabs").getByRole("tab",{name:"線路"}));fireEvent.change(screen.getByLabelText("接続元の駅"),{target:{value:"20"}});fireEvent.change(screen.getByLabelText("接続先の駅"),{target:{value:"10"}});fireEvent.click(screen.getByRole("button",{name:"線路を接続"}));await screen.findByText("線路を接続しました。");await ready();fireEvent.click(screen.getByRole("tab",{name:"建設"}));fireEvent.click(region(".editor-tabs").getByRole("tab",{name:"駅設置"}));undo();await screen.findByText(/駅設置Undo不可: 接続線路/);await ready();expect(vi.mocked(invoke).mock.calls.some(([command])=>command==="delete_station")).toBe(false);expect((region(".station-history").getByRole("button",{name:"駅設置を取り消す"}) as HTMLButtonElement).disabled).toBe(false);
});
test("text inputs keep native undo and track drafts have shortcut priority",async()=>{
 await start();await place();fireEvent.keyDown(region("#panel-stations").getByLabelText("ホーム長 (20m級・両相当)"),{key:"z",ctrlKey:true});expect(vi.mocked(invoke).mock.calls.some(([command])=>command==="delete_station")).toBe(false);
 fireEvent.click(region(".editor-tabs").getByRole("tab",{name:"線路"}));const map=screen.getByRole("img",{name:"駅配置用OpenStreetMap"});map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});fireEvent.click(map,{clientX:200,clientY:130});undo();expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);redo();expect(map.querySelectorAll('.draft-anchor')).toHaveLength(1);expect(vi.mocked(invoke).mock.calls.some(([command])=>command==="delete_station")).toBe(false);
});
test("world load ends history and a new placement discards redo",async()=>{
 await start();await place();undo();await screen.findByText(/直前の駅設置を取り消しました/);await ready();await place();redo();await screen.findByText("やり直せる駅設置履歴はありません。");await ready();expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==="place_station_on_alignment")).toHaveLength(2);
 fireEvent.click(region(".save-controls").getByRole("button",{name:"読込"}));fireEvent.click(region(".load-confirm").getByRole("button",{name:"読込を続ける"}));await screen.findByText("保存したワールドを読み込みました。");await ready();undo();await screen.findByText(/取り消せる駅設置履歴はありません/);await ready();expect(region(".station-directory").getByRole("button",{name:/^既存駅 /})).toBeTruthy();
});
test("repeated keydown cannot issue duplicate station deletion",async()=>{
 await start();await place();const implementation=vi.mocked(invoke).getMockImplementation()!;let finish!:(value:unknown)=>void;
 vi.mocked(invoke).mockImplementation((command,args)=>command==="delete_station"?new Promise(resolve=>{finish=resolve;}):implementation(command,args));
 undo();undo();fireEvent.keyDown(document.body,{key:"z",ctrlKey:true,repeat:true});expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==="delete_station")).toHaveLength(1);finish(initial);await screen.findByText(/直前の駅設置を取り消しました/);await ready();
});

test("multiple placements undo and redo in LIFO order",async()=>{
 await start();await place();await place();undo();await screen.findByText(/直前の駅設置を取り消しました/);await ready();undo();await waitFor(()=>expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==="delete_station")).toHaveLength(2));
 expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==="delete_station").map(([,args])=>(args as any).stationId)).toEqual(["21","20"]);
 await waitFor(()=>expect((region(".station-history").getByRole("button",{name:"駅設置をやり直す"}) as HTMLButtonElement).disabled).toBe(false));redo();await screen.findByText(/駅を再設置しました/);await ready();redo();await waitFor(()=>expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==="place_station_on_alignment")).toHaveLength(4));
 await waitFor(()=>expect((region(".station-history").getByRole("button",{name:"駅設置を取り消す"}) as HTMLButtonElement).disabled).toBe(false));undo();await screen.findByText(/直前の駅設置を取り消しました/);await ready();expect(invoke).toHaveBeenCalledWith("delete_station",{stationId:"23"});
});
test("failed world load retains the station history",async()=>{
 const failures=await start();await place();failures.load=true;fireEvent.click(region(".save-controls").getByRole("button",{name:"読込"}));fireEvent.click(region(".load-confirm").getByRole("button",{name:"読込を続ける"}));await screen.findByText(/load failure/);await ready();undo();await screen.findByText(/直前の駅設置を取り消しました/);await ready();expect(invoke).toHaveBeenCalledWith("delete_station",{stationId:"20"});
});
test.each(["name","size"])("successful station %s edit ends placement history",async kind=>{
 await start();await place();fireEvent.click(region(".station-directory").getByRole("button",{name:/^新駅 /}));
 if(kind==="name"){fireEvent.change(screen.getByLabelText("選択駅の名前"),{target:{value:"改名駅"}});fireEvent.click(screen.getByRole("button",{name:"駅名を適用"}));await screen.findByText(/駅名を変更しました/);await ready();}
 else{fireEvent.change(screen.getByLabelText("選択駅のホーム長 (20m級・両相当)"),{target:{value:"12"}});fireEvent.click(screen.getByRole("button",{name:"ホーム寸法を適用"}));await screen.findByText("ホーム寸法を変更しました。保存でファイルに反映します。");await ready();}
 undo();await screen.findByText(/取り消せる駅設置履歴はありません/);await ready();expect(vi.mocked(invoke).mock.calls.some(([command])=>command==="delete_station")).toBe(false);
});
test("RoutePath membership reference blocks station undo even without Track connections",async()=>{
 await start();await place();fireEvent.click(screen.getByRole("tab",{name:"路線"}));fireEvent.change(screen.getByLabelText("候補の起点"),{target:{value:"10"}});fireEvent.change(screen.getByLabelText("候補の終点"),{target:{value:"20"}});fireEvent.click(screen.getByRole("button",{name:"経路候補を検索"}));await screen.findByLabelText("候補から作る路線名");fireEvent.change(screen.getByLabelText("候補から作る路線名"),{target:{value:"参照路線"}});fireEvent.click(screen.getByRole("button",{name:"候補を新規路線へ適用"}));await screen.findByText(/路線と所属駅を適用しました/);await ready();
 fireEvent.click(screen.getByRole("tab",{name:"建設"}));fireEvent.click(region(".editor-tabs").getByRole("tab",{name:"駅設置"}));undo();await screen.findByText(/駅設置Undo不可: 路線: 参照路線/);await ready();expect(vi.mocked(invoke).mock.calls.some(([command])=>command==="delete_station")).toBe(false);
});
