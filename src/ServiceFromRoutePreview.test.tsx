// @vitest-environment jsdom
import React from "react";
import {afterEach,expect,test,vi} from "vitest";
import {cleanup,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {invoke} from "@tauri-apps/api/core";
import {ServiceFromRoutePreview,trackRouteOccurrences,type ServiceRoutePreview} from "./ServiceFromRoutePreview";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));afterEach(()=>{cleanup();vi.resetAllMocks();});
const stations=[{id:"1",name:"中央"},{id:"2",name:"途中"},{id:"3",name:"終点"}],tracks=[{id:"4",from:"1",to:"2"},{id:"5",from:"2",to:"3"}],routes=[{id:"6",name:"路線A",start:"1",tracks:["4","5"]}];
const preview:ServiceRoutePreview={route_id:"6",start:"1",tracks:["4","5"],stations:stations.map((station,index)=>({station_id:station.id,occurrence:index,route_occurrence:index,route_member:index!==1})),requires_stop_selection:true};
function start(canApply=true){vi.mocked(invoke).mockResolvedValue(preview);const props={routes,tracks,routePaths:[],stations,worldVersion:{},enabled:true,canApply,onApply:vi.fn()};const view=render(<ServiceFromRoutePreview {...props}/>);const scope=within(screen.getByRole("region",{name:"路線区間から新規系統下書き"}));fireEvent.change(scope.getByLabelText("元にする路線"),{target:{value:"6"}});return {...view,props,scope};}
async function inspect(scope:ReturnType<typeof within>){fireEvent.click(scope.getByRole("button",{name:"系統用の区間を確認"}));await scope.findByText(/元路線2番目 \/ 路線所属外/);}
test("all-stop policy explicitly includes non-members and requires confirmation before draft import",async()=>{
 const {props,scope}=start();await inspect(scope);expect(invoke).toHaveBeenCalledWith("preview_service_from_route",{routeId:"6",fromOccurrence:0,toOccurrence:2});expect(props.onApply).not.toHaveBeenCalled();const button=scope.getByRole("button",{name:"新規系統の下書きへ取り込む"});expect((button as HTMLButtonElement).disabled).toBe(true);
 fireEvent.click(scope.getByLabelText("全駅停車（路線所属外も含む）"));expect((scope.getByLabelText("候補停車 2: 途中") as HTMLInputElement).checked).toBe(true);expect((button as HTMLButtonElement).disabled).toBe(true);fireEvent.click(scope.getByLabelText("所属外駅を含め、表示の停車・通過を確認しました"));fireEvent.click(button);expect(props.onApply).toHaveBeenCalledWith({start:"1",tracks:["4","5"],stops:[0,1,2]});expect(vi.mocked(invoke).mock.calls.every(([command])=>command==="preview_service_from_route")).toBe(true);
});
test("custom stop selection protects endpoints and changing stops revokes confirmation",async()=>{
 const {props,scope}=start();await inspect(scope);fireEvent.click(scope.getByLabelText("各駅を個別指定（初期表示は全駅停車）"));expect((scope.getByLabelText("候補停車 1: 中央") as HTMLInputElement).disabled).toBe(true);expect((scope.getByLabelText("候補停車 3: 終点") as HTMLInputElement).disabled).toBe(true);
 const confirmation=scope.getByLabelText("所属外駅を含め、表示の停車・通過を確認しました");fireEvent.click(confirmation);fireEvent.click(scope.getByLabelText("候補停車 2: 途中"));expect((confirmation as HTMLInputElement).checked).toBe(false);fireEvent.click(confirmation);fireEvent.click(scope.getByRole("button",{name:"新規系統の下書きへ取り込む"}));expect(props.onApply).toHaveBeenCalledWith({start:"1",tracks:["4","5"],stops:[0,2]});
});
test("existing draft guard prevents import and outdated preview completions are discarded",async()=>{
 const {props,scope,rerender}=start(false);await inspect(scope);fireEvent.click(scope.getByLabelText("全駅停車（路線所属外も含む）"));fireEvent.click(scope.getByLabelText("所属外駅を含め、表示の停車・通過を確認しました"));expect((scope.getByRole("button",{name:"新規系統の下書きへ取り込む"}) as HTMLButtonElement).disabled).toBe(true);expect(props.onApply).not.toHaveBeenCalled();
 let finish!:(response:ServiceRoutePreview)=>void;vi.mocked(invoke).mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;}));fireEvent.click(scope.getByRole("button",{name:"系統用の区間を確認"}));rerender(<ServiceFromRoutePreview {...props} worldVersion={{}}/>);finish(preview);await waitFor(()=>expect(scope.queryByText(/元路線2番目 \/ 路線所属外/)).toBeNull());
});
test("occurrence walk preserves revisits, sidecar tracks and excludes Alignment paths",()=>{
 expect(trackRouteOccurrences({...routes[0],tracks:["4","4","4"]},tracks)).toEqual(["1","2","1","2"]);expect(trackRouteOccurrences({...routes[0],tracks:[]},tracks,{route_id:"6",segments:[{kind:"track",track_id:"4"}]})).toEqual(["1","2"]);expect(trackRouteOccurrences(routes[0],tracks,{route_id:"6",segments:[{kind:"alignment",alignment_id:"9",from:"1",to:"3"}]})).toBeNull();expect(trackRouteOccurrences({...routes[0],tracks:["missing"]},tracks)).toBeNull();
});
test("reverse occurrence selection is forwarded and displayed in reverse order",async()=>{
 const {props,scope}=start();fireEvent.change(scope.getByLabelText("区間の出発位置"),{target:{value:"2"}});fireEvent.change(scope.getByLabelText("区間の終点位置"),{target:{value:"0"}});vi.mocked(invoke).mockResolvedValueOnce({...preview,start:"3",tracks:["5","4"],stations:[...preview.stations].reverse().map((station,index)=>({...station,occurrence:index}))});await inspect(scope);expect(invoke).toHaveBeenCalledWith("preview_service_from_route",{routeId:"6",fromOccurrence:2,toOccurrence:0});fireEvent.click(scope.getByLabelText("全駅停車（路線所属外も含む）"));fireEvent.click(scope.getByLabelText("所属外駅を含め、表示の停車・通過を確認しました"));fireEvent.click(scope.getByRole("button",{name:"新規系統の下書きへ取り込む"}));expect(props.onApply).toHaveBeenCalledWith({start:"3",tracks:["5","4"],stops:[0,1,2]});
});
test("mismatched response Track IDs are never offered for import",async()=>{
 const {props,scope}=start();vi.mocked(invoke).mockResolvedValueOnce({...preview,tracks:["5","4"]});fireEvent.click(scope.getByRole("button",{name:"系統用の区間を確認"}));await scope.findByText(/経路プレビューの形式が一致しません/);expect(props.onApply).not.toHaveBeenCalled();expect(scope.queryByRole("button",{name:"新規系統の下書きへ取り込む"})).toBeNull();
});
test("endpoint changes discard delayed responses and non-member endpoint remains stop-fixed",async()=>{
 const {scope}=start();let finish!:(response:ServiceRoutePreview)=>void;vi.mocked(invoke).mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;}));fireEvent.click(scope.getByRole("button",{name:"系統用の区間を確認"}));fireEvent.change(scope.getByLabelText("区間の終点位置"),{target:{value:"1"}});finish(preview);await waitFor(()=>expect(scope.queryByText(/元路線2番目 \/ 路線所属外/)).toBeNull());
 vi.mocked(invoke).mockResolvedValueOnce({...preview,tracks:["4"],stations:preview.stations.slice(0,2)});await inspect(scope);fireEvent.click(scope.getByLabelText("各駅を個別指定（初期表示は全駅停車）"));const endpoint=scope.getByLabelText("候補停車 2: 途中") as HTMLInputElement;expect(endpoint.checked).toBe(true);expect(endpoint.disabled).toBe(true);expect(invoke).toHaveBeenLastCalledWith("preview_service_from_route",{routeId:"6",fromOccurrence:0,toOccurrence:1});
});
