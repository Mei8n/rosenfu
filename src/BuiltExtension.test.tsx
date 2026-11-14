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
for(const endpoint of ["start","end"] as const)test(`${endpoint} extension joins existing ID inside a multi-path plan and preserves station dimensions`,async()=>{
 const map=await start();const click=(x:number,y=130)=>fireEvent.click(map,{clientX:x,clientY:y,detail:1});
 const anchor=endpoint==="start"?180:250,finish=endpoint==="start"?120:310;
 click(anchor+2);expect(screen.getByText(/から延伸を計画しています/)).toBeTruthy();click(finish);
 fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));fireEvent.change(screen.getByLabelText("ホーム長 (20m級・両相当)"),{target:{value:"8"}});click((anchor+finish)/2);
 fireEvent.click(screen.getByRole("tab",{name:"線路"}));click(finish);click(340,220);click(400,220);
 vi.mocked(invoke).mockRejectedValueOnce(Error("atomic failure"));fireEvent.click(screen.getByRole("button",{name:"計画を着工"}));await screen.findByText(/atomic failure/);
 const request=vi.mocked(invoke).mock.calls.find(([c])=>c==="build_plan")![1] as {alignments:any[]};
 expect(request.alignments).toHaveLength(2);const ext=request.alignments.find(p=>p.extension);
 expect(ext.extension).toEqual({alignmentId:"3",endpoint});expect(ext.points[0]).toEqual(line.points[endpoint==="start"?0:1]);expect(ext.stations[0].lengthM).toBe(160);
 expect(map.querySelectorAll(".planned-station")).toHaveLength(1);
 vi.mocked(invoke).mockResolvedValueOnce(world);fireEvent.click(screen.getByRole("button",{name:"計画を着工"}));await waitFor(()=>expect(map.querySelectorAll(".draft-anchor")).toHaveLength(0));
});
test("attributed endpoints can start an extension without duplicating the alignment",async()=>{
 const map=await start({alignment_provenance:[{alignment_id:"3",source:{source_key:"x"}}]});
 fireEvent.click(map,{clientX:250,clientY:130});expect(await screen.findByText(/から延伸を計画しています/)).toBeTruthy();expect(map.querySelectorAll(".draft-anchor")).toHaveLength(1);
});

test('extending invalidates built station redo ratios from the old alignment length',async()=>{
 const map=await start();
 const station={id:"4",owner:"2",name:"駅",...point(215,130)};
 const placement={station_id:"4",alignment_id:"3",position:0.5,length_m:200,width_m:12};
 fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));
 vi.mocked(invoke).mockResolvedValueOnce({...world,stations:[station],station_placements:[placement]});
 fireEvent.click(map,{clientX:215,clientY:130});await waitFor(()=>expect(invoke).toHaveBeenCalledWith("place_station_on_alignment",expect.anything()));
 await screen.findByText(/を線路に沿って配置しました/);
 vi.mocked(invoke).mockResolvedValueOnce(world);fireEvent.keyDown(document.body,{key:"z",ctrlKey:true});await screen.findByText(/直前の駅設置を取り消しました/);
 fireEvent.click(screen.getByRole("tab",{name:"線路"}));fireEvent.click(map,{clientX:250,clientY:130});fireEvent.click(map,{clientX:310,clientY:130});
 vi.mocked(invoke).mockResolvedValueOnce({...world,alignments:[{...line,points:[...line.points,point(310,130)]}]});fireEvent.click(screen.getByRole("button",{name:"計画を着工"}));await screen.findByText(/線路を建設しました/);
 fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));const count=vi.mocked(invoke).mock.calls.length;
 fireEvent.keyDown(document.body,{key:"Z",ctrlKey:true,shiftKey:true});await screen.findByText(/やり直せる駅設置履歴はありません/);expect(vi.mocked(invoke).mock.calls).toHaveLength(count);
});

test('automatic extension rejects a backward join in preview',async()=>{
 const map=await start();
 fireEvent.click(map,{clientX:250,clientY:130});fireEvent.pointerMove(map,{clientX:220,clientY:190});
 expect(await screen.findByText(/線路が折り返しています/)).toBeTruthy();expect(map.querySelector('.invalid-candidate')).toBeTruthy();
 fireEvent.click(map,{clientX:220,clientY:190});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(1);
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(true);
 fireEvent.click(map,{clientX:310,clientY:130});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
});

test('an alignment used by a service can start and submit an extension',async()=>{
 const map=await start({service_paths:[{service_id:"8",next_leg_key:"2",legs:[{key:"1",segment:{kind:"alignment",alignment_id:"3",from:"4",to:"5"}}],stops:[]}]});
 fireEvent.click(map,{clientX:250,clientY:130});expect(screen.getByText(/から延伸を計画しています/)).toBeTruthy();
 fireEvent.click(map,{clientX:310,clientY:130});vi.mocked(invoke).mockResolvedValueOnce(world);
 fireEvent.click(screen.getByRole("button",{name:"計画を着工"}));await waitFor(()=>expect(invoke).toHaveBeenCalledWith('build_plan',expect.objectContaining({alignments:expect.arrayContaining([expect.objectContaining({extension:{alignmentId:'3',endpoint:'end'}})])})));
});

test('extension inherits non-default design speed and prevents changing its setting',async()=>{
 const map=await start({alignments:[{...line,design_speed_kmh:95}]});
 fireEvent.click(map,{clientX:250,clientY:130});
 const input=screen.getByRole('spinbutton',{name:'線路の設計最高速度'}) as HTMLInputElement;
 expect(input.value).toBe('95');expect(input.disabled).toBe(true);
 fireEvent.click(map,{clientX:310,clientY:133});
 vi.mocked(invoke).mockResolvedValueOnce(world);fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('build_plan',expect.objectContaining({alignments:expect.arrayContaining([expect.objectContaining({designSpeedKmh:95})])})));
});
