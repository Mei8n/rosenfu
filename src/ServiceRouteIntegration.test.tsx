// @vitest-environment jsdom
import React from "react";
import {afterEach,expect,test,vi} from "vitest";
import {cleanup,fireEvent,render,screen,within} from "@testing-library/react";
import {invoke,isTauri} from "@tauri-apps/api/core";
import {App} from "./App";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn(),isTauri:vi.fn()}));afterEach(()=>{cleanup();vi.resetAllMocks();});
const world={next_id:"8",players:[{id:"100",name:"Player"}],companies:[{id:"200",owner:"100",name:"Railway"}],stations:[{id:"1",owner:"200",name:"中央",latitude:35.67,longitude:139.75},{id:"2",owner:"200",name:"途中",latitude:35.68,longitude:139.75},{id:"3",owner:"200",name:"終点",latitude:35.69,longitude:139.75}],tracks:[{id:"4",owner:"200",from:"1",to:"2"},{id:"5",owner:"200",from:"2",to:"3"}],routes:[{id:"6",owner:"200",name:"参考路線",start:"1",tracks:[]}],route_paths:[{route_id:"6",segments:[{kind:"track",track_id:"4"},{kind:"track",track_id:"5"}],member_stations:["1","3"]}],services:[{id:"7",owner:"200",name:"既存系統",start:"1",tracks:["4","5"],stops:[0,2]}]};
test("route section import is a new named draft only and never overwrites an existing service",async()=>{
 vi.mocked(isTauri).mockReturnValue(true);let rejectSave=true;
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command==="engine_info")return {name:"Rosenfu",version:"service-route-test"};if(command==="world_snapshot")return world;
  if(command==="preview_service_from_route")return {route_id:"6",start:"1",tracks:["4","5"],stations:[{station_id:"1",occurrence:0,route_occurrence:0,route_member:true},{station_id:"2",occurrence:1,route_occurrence:1,route_member:false},{station_id:"3",occurrence:2,route_occurrence:2,route_member:true}],requires_stop_selection:true};
  if(command==="save_service"){if(rejectSave){rejectSave=false;throw Error("service save rejected");}const data=args as any;return {...world,next_id:"9",services:[...world.services,{id:"8",owner:"200",name:data.name,start:data.start,tracks:data.tracks,stops:data.stops}]};}throw Error(command);
 });render(<App/>);await screen.findByText(/service-route-test/);fireEvent.click(screen.getByRole("tab",{name:"系統"}));const pane=within(document.getElementById('workspace-panel-services')!);const wizard=within(pane.getByRole('region',{name:'路線区間から新規系統下書き'}));
 fireEvent.click(pane.getByRole('button',{name:'系統編集: 既存系統'}));fireEvent.change(wizard.getByLabelText('元にする路線'),{target:{value:'6'}});fireEvent.click(wizard.getByRole('button',{name:'系統用の区間を確認'}));await wizard.findByText(/元路線2番目 \/ 路線所属外/);
 fireEvent.click(wizard.getByLabelText('各駅を個別指定（初期表示は全駅停車）'));fireEvent.click(wizard.getByLabelText('候補停車 2: 途中'));fireEvent.click(wizard.getByLabelText('所属外駅を含め、表示の停車・通過を確認しました'));
 expect((wizard.getByRole('button',{name:'新規系統の下書きへ取り込む'}) as HTMLButtonElement).disabled).toBe(true);expect((pane.getByLabelText('系統名') as HTMLInputElement).value).toBe('既存系統');
 fireEvent.click(pane.getByRole('button',{name:'新規系統'}));fireEvent.click(wizard.getByRole('button',{name:'新規系統の下書きへ取り込む'}));expect((pane.getByLabelText('系統名') as HTMLInputElement).value).toBe('');expect((pane.getByLabelText('停車 2: 途中') as HTMLInputElement).checked).toBe(false);expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='save_service')).toBe(false);
 fireEvent.change(pane.getByLabelText('系統名'),{target:{value:'独立した系統名'}});fireEvent.click(pane.getByRole('button',{name:'系統を適用'}));await screen.findByText(/service save rejected/);expect((pane.getByLabelText('系統名') as HTMLInputElement).value).toBe('独立した系統名');expect((pane.getByLabelText('停車 2: 途中') as HTMLInputElement).checked).toBe(false);
 fireEvent.click(pane.getByRole('button',{name:'系統を適用'}));await within(document.querySelector('.app-footer')!).findByText(/運行系統をワールドに適用しました/);expect(invoke).toHaveBeenCalledWith('save_service',{serviceId:null,name:'独立した系統名',start:'1',tracks:['4','5'],stops:[0,2]});expect(pane.getByRole('button',{name:'系統編集: 既存系統'})).toBeTruthy();expect(pane.getByRole('button',{name:'系統編集: 独立した系統名'})).toBeTruthy();
});
