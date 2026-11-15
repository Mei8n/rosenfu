import './testConstructionEnvironment';
// @vitest-environment jsdom
import React from "react";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { App } from "./App";
import { project, DEFAULT_CAMERA } from "./map";
import { alignmentLength, pointOnAlignment, stationPolygon } from "./alignment";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
window.PointerEvent=MouseEvent as typeof PointerEvent;
afterEach(() => { localStorage.removeItem("rosenfu.display-settings"); cleanup(); vi.resetAllMocks(); });
const alignment = { id:"3",owner:"2",name:"試験線",points:[{latitude:35.67504,longitude:139.72},{latitude:35.67504,longitude:139.78}] };
const empty = {next_id:"3",players:[{id:"1",name:"Player"}],companies:[{id:"2",owner:"1",name:"Railway"}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
const built = {...empty,next_id:"4",alignments:[alignment]};
const placed = {...built,next_id:"5",stations:[{id:"4",owner:"2",name:"新駅",latitude:35.67504,longitude:139.75}],station_placements:[{station_id:"4",alignment_id:"3",position:.5,length_m:200,width_m:12}]};
async function start(snapshot: unknown = empty) {
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke).mockImplementation(async command => {
    if(command==="engine_info") return {name:"Rosenfu",version:"construction-test"};
    if(command==="world_snapshot") return snapshot;
    if(command==="list_save_slots") return {active_slot_id:"test-slot",slots:[{id:"test-slot",name:"試験",loadable:true}]};
    if(command==="autosave_status") return {available:false};
    throw Error("unexpected "+command);
  });
  render(<App/>); fireEvent.click(screen.getByRole("button",{name:"自作サンプル（オフライン）"})); await screen.findByText(/construction-test/);
  const map=screen.getByRole("img",{name:"駅配置用サンプル地図"});
  map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
  return map;
}
test("construction starts with tracks and drafts retain points after failed build",async()=>{
  const map=await start();
  expect(screen.getByRole("tab",{name:"線路"}).getAttribute("aria-selected")).toBe("true");
  expect(screen.queryByLabelText("建設する線路名")).toBeNull();

  fireEvent.click(map,{clientX:180,clientY:130}); fireEvent.click(map,{clientX:250,clientY:160}); fireEvent.click(map,{clientX:320,clientY:130});
  expect(map.querySelectorAll(".draft-anchor")).toHaveLength(3);
  expect((screen.getByRole("button",{name:"保存"}) as HTMLButtonElement).disabled).toBe(false);
  vi.mocked(invoke).mockRejectedValueOnce(Error("build rejected"));
  fireEvent.click(screen.getByRole("button",{name:"計画を着工"}));
  await screen.findByText(/build rejected/);
  expect(map.querySelectorAll(".draft-anchor")).toHaveLength(3);
  const call=vi.mocked(invoke).mock.calls.find(([command])=>command==="build_alignment")!;
  expect((call[1] as {points:unknown[]}).points.length).toBeGreaterThan(3);
  expect((call[1] as {name:string}).name).toBe("新しい線路");
  vi.mocked(invoke).mockResolvedValueOnce(built);
  fireEvent.click(screen.getByRole("button",{name:"計画を着工"}));
  await screen.findByText(/線路を建設しました/);
  expect(map.querySelectorAll(".draft-anchor")).toHaveLength(0);
  expect(map.querySelectorAll(".alignment")).toHaveLength(1);
});
test("draft undo, cancel and load confirmation preserve intentional construction",async()=>{
  const map=await start();

  fireEvent.click(map,{clientX:200,clientY:130});fireEvent.click(map,{clientX:300,clientY:170});
  fireEvent.keyDown(document.body,{key:"Backspace"});
  expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
  expect(screen.queryByText("Backspace")).toBeNull();
  expect(screen.queryByText("移動モード")).toBeNull();
  // Shortcuts still bubble from focused toolbar controls after map editing.
  const drawButton=screen.getByRole("tab",{name:"線路"});
  drawButton.focus();
  fireEvent.keyDown(drawButton,{key:"z",ctrlKey:true});
  expect(map.querySelectorAll(".draft-anchor")).toHaveLength(1);
  fireEvent.keyDown(drawButton,{key:"Z",ctrlKey:true,shiftKey:true});
  expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
  fireEvent.keyDown(drawButton,{key:"z",ctrlKey:true});
  expect(map.querySelectorAll(".draft-anchor")).toHaveLength(1);
  fireEvent.keyDown(document.body,{key:"Escape"});fireEvent.click(map,{clientX:300,clientY:170});
  expect(map.querySelectorAll(".draft-anchor")).toHaveLength(1);
  fireEvent.click(screen.getByRole("button",{name:"読込"}));
  expect(screen.getByRole("button",{name:"読込を続ける"})).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"戻る"}));
  expect(map.querySelectorAll(".draft-anchor")).toHaveLength(1);
  fireEvent.click(screen.getByRole("tab",{name:"線路"}));
  fireEvent.click(screen.getByRole("button",{name:"下書きを消去"}));
  expect(map.querySelectorAll(".draft-anchor")).toHaveLength(0);
});
test("line-constrained station previews then commits metric platform dimensions",async()=>{
  const map=await start(built);
  fireEvent.click(screen.getByRole("tab",{name:"建設"}));fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));
  expect(screen.queryByLabelText("新しい駅名")).toBeNull();expect(screen.queryByLabelText("幅 (m)")).toBeNull();
  fireEvent.change(screen.getByLabelText("駅の長さ"),{target:{value:"12"}});
  fireEvent.pointerMove(map,{clientX:250,clientY:150});
  expect(map.querySelector(".station-platform")).toBeTruthy();
  expect(invoke).not.toHaveBeenCalledWith("place_station_on_alignment",expect.anything());
  vi.mocked(invoke).mockResolvedValueOnce({...placed,station_placements:[{...placed.station_placements[0],length_m:240}]});
  fireEvent.click(map,{clientX:250,clientY:150});
  fireEvent.click(screen.getByRole("button",{name:"計画を着工"}));await screen.findByText("計画駅を建設しました。");
  expect(invoke).toHaveBeenCalledWith("build_plan",expect.objectContaining({stations:[expect.objectContaining({alignmentId:"3",lengthM:240,widthM:12})]}));
  expect(map.querySelector(".station-marker circle.station-dot")).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"拡大"}));fireEvent.click(screen.getByRole("button",{name:"拡大"}));fireEvent.click(screen.getByRole("button",{name:"拡大"}));
  await waitFor(()=>expect(map.querySelector(".station-marker path.station-platform")).toBeTruthy());
  expect(map.querySelector(".station-marker circle")).toBeNull();
});
test("placement rejects distant clicks and keeps station operation uncalled",async()=>{
 const map=await start(built);fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));
 fireEvent.click(map,{clientX:250,clientY:20});await screen.findByText(/処理できませんでした: 駅は線路上に置いてください/);
 expect(invoke).not.toHaveBeenCalledWith("place_station_on_alignment",expect.anything());
 fireEvent.click(screen.getByRole("button",{name:"エラー通知を閉じる"}));
 expect(screen.queryByText(/処理できませんでした/)).toBeNull();
});
test("existing platform resize preserves ID and duplicate submits are blocked",async()=>{
  const map=await start(placed);fireEvent.click(screen.getByRole("tab",{name:"建設"}));fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));
  fireEvent.click(document.querySelector(".station-entry")!);
  fireEvent.change(screen.getByLabelText("選択駅の長さ"),{target:{value:"16"}});
  let finish!: (value:unknown)=>void;
  vi.mocked(invoke).mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;}));
  expect(screen.queryByRole("button",{name:"ホーム寸法を適用"})).toBeNull();
  await waitFor(()=>expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==="update_station_placement")).toHaveLength(1));
  expect(invoke).toHaveBeenCalledWith("update_station_placement",{stationId:"4",lengthM:320,widthM:12});
  finish({...placed,station_placements:[{...placed.station_placements[0],length_m:320}]});
  await waitFor(()=>expect(screen.getByText("ホーム寸法を変更しました。保存でファイルに反映します。")).toBeTruthy());
  expect(screen.getByText("未保存の変更あり")).toBeTruthy();
});

test("keyboard position previews a platform and too-long platform is blocked",async()=>{
 const short={...built,alignments:[{...alignment,points:[{latitude:35.67504,longitude:139.749},{latitude:35.67504,longitude:139.751}]}]};
 const map=await start(short);fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));
 fireEvent.keyDown(map,{key:"ArrowRight"});
 expect(screen.getAllByText("この線路に収まるホーム長を指定してください。").length).toBeGreaterThan(0);
 fireEvent.keyDown(map,{key:"Enter"});
 expect(invoke).not.toHaveBeenCalledWith("place_station_on_alignment",expect.anything());
 fireEvent.change(screen.getByLabelText("駅の長さ"),{target:{value:"5"}});
 expect(map.querySelector(".station-platform")).toBeTruthy();
 vi.mocked(invoke).mockResolvedValueOnce(placed);
 fireEvent.keyDown(map,{key:"Enter"});
 fireEvent.click(screen.getByRole("button",{name:"計画を着工"}));await screen.findByText("計画駅を建設しました。");
});

test("plan redo, duplicate rejection and display controls",async()=>{
 const map=await start();
 fireEvent.click(map,{clientX:180,clientY:130});fireEvent.click(map,{clientX:250,clientY:160});
 fireEvent.keyDown(document.body,{key:"z",ctrlKey:true}); expect(map.querySelectorAll(".draft-anchor")).toHaveLength(1);
 fireEvent.keyDown(document.body,{key:"Z",ctrlKey:true,shiftKey:true}); expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
 fireEvent.click(map,{clientX:250,clientY:160});expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
 expect(screen.queryByText("敷設不可：重複する点があります")).toBeNull();
 expect((screen.getByRole("button",{name:"計画を着工"}) as HTMLButtonElement).disabled).toBe(false);
 fireEvent.click(screen.getByRole("button",{name:"ダークに切替"}));expect(document.querySelector("main")?.getAttribute("data-theme")).toBe("dark");
 fireEvent.change(screen.getByLabelText("線の太さ"),{target:{value:"6"}});expect(document.querySelector("main")?.style.getPropertyValue("--rail-width")).toBe("6");
});

test("20m equivalent inputs preserve fractional metre lengths when creating and editing",async()=>{
 const snapshot={...placed,station_placements:[{...placed.station_placements[0],length_m:231.5,width_m:17}]};
 await start(snapshot);fireEvent.click(screen.getByRole("tab",{name:"建設"}));fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));fireEvent.click(screen.getByRole("tab",{name:"駅"}));fireEvent.click(screen.getByRole("button",{name:/^新駅 35/}));
 expect(screen.queryByLabelText("選択駅のホーム長 (m)")).toBeNull();
 expect((screen.getByLabelText("選択駅の長さ") as HTMLInputElement).value).toBe("11.575");
 fireEvent.change(screen.getByLabelText("選択駅の長さ"),{target:{value:"12.125"}});
 expect((screen.getByLabelText("選択駅の長さ") as HTMLInputElement).value).toBe("12.125");
 vi.mocked(invoke).mockResolvedValueOnce({...snapshot,station_placements:[{...snapshot.station_placements[0],length_m:242.5}]});
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith("update_station_placement",{stationId:"4",lengthM:242.5,widthM:17}));
 fireEvent.click(screen.getByRole("tab",{name:"建設"}));fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));
 fireEvent.change(screen.getByLabelText("駅の長さ"),{target:{value:"2.25"}});
 expect((screen.getByLabelText("駅の長さ") as HTMLInputElement).value).toBe("2.25");
});

test("station display switches exactly across ZL12 and ZL13 without changing dimensions",async()=>{
 const map=await start(placed);
 const zoomIn=()=>fireEvent.click(screen.getByRole("button",{name:"拡大"}));
 await screen.findByText("ZL 12 / Web Mercator");
 expect(map.querySelector(".station-marker circle.station-dot")).toBeTruthy();
 expect(map.querySelector(".station-marker path.station-platform")).toBeNull();
 zoomIn();await screen.findByText("ZL 13 / Web Mercator");
 await waitFor(()=>expect(map.querySelector(".station-marker circle.station-dot")).toBeNull());
 expect(map.querySelector(".station-marker path.station-platform")).toBeTruthy();
 fireEvent.click(screen.getByRole("button",{name:"縮小"}));
 await screen.findByText("ZL 12 / Web Mercator");
 expect(map.querySelector(".station-marker circle.station-dot")).toBeTruthy();
 expect(vi.mocked(invoke).mock.calls.some(([command])=>command==="update_station_placement")).toBe(false);
});
test("low-zoom endpoint previews use the full-platform corrected center",async()=>{
 const map=await start(built);fireEvent.click(screen.getByRole("tab",{name:"建設"}));fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));
 fireEvent.change(screen.getByLabelText("駅の長さ"),{target:{value:"12"}});
 for(const position of [0,1]){
  const hover=project(alignment.points[position].latitude,alignment.points[position].longitude,DEFAULT_CAMERA);fireEvent.pointerMove(map,{clientX:hover.x/2,clientY:hover.y/2});
  const preview=map.querySelector("path.station-placement-preview")!;
  expect(map.querySelector("circle.station-platform")).toBeNull();
  const polygon=stationPolygon(alignment.points,position,240,12);
  const expected=polygon.map(p=>project(p.latitude,p.longitude,DEFAULT_CAMERA));
  expect(preview.getAttribute("d")).toBe(expected.map((p,i)=>`${i?"L":"M"}${p.x},${p.y}`).join(" ")+"Z");
 }
});
test("reference name is automatic and does not place or rename until confirmed",async()=>{
 const map=await start(placed);fireEvent.click(screen.getByRole("tab",{name:"建設"}));fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));fireEvent.click(screen.getByRole("tab",{name:"駅"}));fireEvent.click(screen.getByRole("button",{name:/^新駅 /}));expect(screen.queryByLabelText("新しい駅名")).toBeNull();
 fireEvent.click(screen.getByRole("tab",{name:"建設"}));fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));
 const pack={format:"rosenfu-reference-stations",version:1,dataset:{id:"names",revision:"1",source:"local fixture",license:"test",bounds:{west:139,east:140,south:35,north:36}},stations:[{osmType:"node",osmId:"1",name:"参考中央",latitude:35.67504,longitude:139.756,kind:"station"}]};
 fireEvent.change(screen.getByLabelText("参照駅JSON"),{target:{files:[{size:500,text:()=>Promise.resolve(JSON.stringify(pack))}]}});await screen.findByText("names / 1 / 1件");
 const hover=project(35.67504,139.756,DEFAULT_CAMERA);fireEvent.pointerMove(map,{clientX:hover.x/2,clientY:hover.y/2});
 expect(await screen.findByText("参考中央",{selector:"strong"})).toBeTruthy();expect((screen.getByLabelText("選択駅の名前") as HTMLInputElement).value).toBe("新駅");expect(vi.mocked(invoke).mock.calls.some(([command])=>command==="place_station_on_alignment"||command==="rename_station")).toBe(false);
 vi.mocked(invoke).mockResolvedValueOnce(placed);fireEvent.click(map,{clientX:hover.x/2,clientY:hover.y/2});
 fireEvent.click(screen.getByRole("button",{name:"計画を着工"}));await waitFor(()=>expect(invoke).toHaveBeenCalledWith("build_plan",expect.objectContaining({stations:[expect.objectContaining({name:"参考中央"})]})));
 expect(vi.mocked(invoke).mock.calls.some(([command])=>command==="rename_station")).toBe(false);
});
test("reference name distance uses corrected platform center at both alignment endpoints",async()=>{
 const map=await start(built);fireEvent.click(screen.getByRole("tab",{name:"建設"}));fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));fireEvent.change(screen.getByLabelText("駅の長さ"),{target:{value:"12"}});
 const half=120/alignmentLength(alignment.points),startPoint=pointOnAlignment(alignment.points,half),endPoint=pointOnAlignment(alignment.points,1-half);
 const pack={format:"rosenfu-reference-stations",version:1,dataset:{id:"endpoint-names",revision:"1",source:"test",license:"test",bounds:{west:139,east:140,south:35,north:36}},stations:[{osmType:"node",osmId:"1",name:"始端中心",...startPoint,kind:"station"},{osmType:"node",osmId:"2",name:"終端中心",...endPoint,kind:"station"}]};
 fireEvent.change(screen.getByLabelText("参照駅JSON"),{target:{files:[{size:1000,text:()=>Promise.resolve(JSON.stringify(pack))}]}});await screen.findByText("endpoint-names / 1 / 2件");
 for(const [position,name] of [["0","始端中心"],["100","終端中心"]]){const point=alignment.points[position==="0"?0:1],hover=project(point.latitude,point.longitude,DEFAULT_CAMERA);fireEvent.pointerMove(map,{clientX:hover.x/2,clientY:hover.y/2});expect(await screen.findByText(name as string,{selector:"strong"})).toBeTruthy();expect(screen.getByText(/約0m・直線距離/)).toBeTruthy();}
});


test("clicking the last point again finishes a draft once without building or duplicate anchors",async()=>{
 const map=await start();
 fireEvent.click(map,{clientX:180,clientY:130,detail:1});
 fireEvent.click(map,{clientX:300,clientY:170,detail:1});
 fireEvent.click(map,{clientX:300,clientY:170,detail:1});
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
 expect(map.classList.contains("placement-enabled")).toBe(true);
 expect((screen.getByRole("button",{name:"計画を着工"}) as HTMLButtonElement).disabled).toBe(false);
 fireEvent.click(map,{clientX:350,clientY:180,detail:1});
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(1);
 expect(map.querySelectorAll(".parked-plan")).toHaveLength(1);
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==="build_alignment")).toBe(false);
 expect((screen.getByRole("button",{name:"計画を着工"}) as HTMLButtonElement).disabled).toBe(true);
});

test("double clicking an existing endpoint does not duplicate it; one point cannot finish",async()=>{
 const map=await start();
 fireEvent.click(map,{clientX:180,clientY:130,detail:1});
 fireEvent.click(map,{clientX:180,clientY:130,detail:2});
 expect(map.classList.contains("placement-enabled")).toBe(true);
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(1);
 fireEvent.click(map,{clientX:300,clientY:170,detail:1});
 fireEvent.click(map,{clientX:300,clientY:170,detail:1});
 fireEvent.click(map,{clientX:300,clientY:170,detail:2});
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
 expect(map.classList.contains("placement-enabled")).toBe(true);
});


test("returning near the endpoint snaps in CSS pixels without a foldback anchor",async()=>{
 const map=await start();
 fireEvent.click(map,{clientX:180,clientY:130,detail:1});fireEvent.click(map,{clientX:300,clientY:170,detail:1});
 fireEvent.pointerMove(map,{clientX:350,clientY:180});fireEvent.pointerMove(map,{clientX:296,clientY:168});
 fireEvent.click(map,{clientX:296,clientY:168,detail:1});
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
 fireEvent.click(map,{clientX:296,clientY:168,detail:2});
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
 fireEvent.click(map,{clientX:100,clientY:240,detail:1});
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(1);
 expect(map.querySelectorAll(".parked-plan")).toHaveLength(1);
 expect(screen.queryByText(/敷設不可：線路が折り返しています/)).toBeNull();
});

test("invalid curve remains diagnostic and cannot add an anchor",async()=>{
 const map=await start();
 fireEvent.click(map,{clientX:180,clientY:130,detail:1});fireEvent.click(map,{clientX:300,clientY:170,detail:1});
 fireEvent.click(map,{clientX:200,clientY:160,detail:1});
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
 expect(map.querySelector(".confirmed-draft")).toBeTruthy();
 expect(map.querySelector(".invalid-candidate")?.getAttribute("d")).toContain("L");
 fireEvent.click(map,{clientX:390,clientY:190,detail:1});
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(3);
 expect(map.querySelector(".invalid-candidate")).toBeNull();
});

test("invalid double click does not finish; selecting a parked plan waits for an explicit endpoint click",async()=>{
 const map=await start();
 fireEvent.click(map,{clientX:180,clientY:130,detail:1});fireEvent.click(map,{clientX:300,clientY:170,detail:1});
 fireEvent.click(map,{clientX:200,clientY:160,detail:1});fireEvent.click(map,{clientX:200,clientY:160,detail:2});
 fireEvent.click(map,{clientX:390,clientY:190,detail:1});
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(3);
 expect(map.querySelectorAll(".parked-plan")).toHaveLength(0);
 fireEvent.click(screen.getByRole("button",{name:"別の線路計画を追加"}));
 fireEvent.click(screen.getByRole("button",{name:/計画 1 を編集/}));
 fireEvent.click(map,{clientX:390,clientY:190,detail:1});
 fireEvent.click(map,{clientX:450,clientY:200,detail:1});
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(4);
});

test("automatic drawing snaps the last anchor to finish and keeps drawing enabled",async()=>{
 const map=await start();
 fireEvent.click(map,{clientX:180,clientY:130,detail:1});fireEvent.click(map,{clientX:300,clientY:170,detail:1});
 fireEvent.click(map,{clientX:296,clientY:168,detail:1});expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
 fireEvent.click(map,{clientX:296,clientY:168,detail:2});fireEvent.click(map,{clientX:100,clientY:240,detail:1});
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(1);expect(map.querySelectorAll(".parked-plan")).toHaveLength(1);
});

test("endpoint snap tolerance stays in CSS pixels after the map resizes",async()=>{
 const map=await start();
 fireEvent.click(map,{clientX:180,clientY:130,detail:1});fireEvent.click(map,{clientX:300,clientY:170,detail:1});
 map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:1000,bottom:600,width:1000,height:600,toJSON:()=>({})});
 fireEvent.click(map,{clientX:595,clientY:337,detail:1});expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
 fireEvent.click(map,{clientX:595,clientY:337,detail:2});expect(map.classList.contains("placement-enabled")).toBe(true);
 fireEvent.click(map,{clientX:100,clientY:400,detail:1});expect(map.querySelectorAll(".parked-plan")).toHaveLength(1);
});

 test("empty redo preserves a finished draft before starting another path", async () => {
 const map = await start();
 fireEvent.click(map, {clientX:180, clientY:130, detail:1});
 fireEvent.click(map, {clientX:300, clientY:170, detail:1});
 fireEvent.click(map, {clientX:300, clientY:170, detail:1});
 fireEvent.keyDown(document.body, {key:"Z", ctrlKey:true, shiftKey:true});
 fireEvent.click(map, {clientX:390, clientY:190, detail:1});
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(1);
 expect(map.querySelectorAll(".parked-plan")).toHaveLength(1);
 });

test("automatic drawing single endpoint finish can resume the same plan without an extra point",async()=>{
 const map=await start();
 fireEvent.click(map,{clientX:180,clientY:130,detail:1});fireEvent.click(map,{clientX:300,clientY:170,detail:1});
 fireEvent.click(map,{clientX:298,clientY:169,detail:1});
 fireEvent.click(map,{clientX:298,clientY:169,detail:1});
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
 fireEvent.click(map,{clientX:390,clientY:190,detail:1});expect(map.querySelectorAll(".draft-anchor")).toHaveLength(3);
 expect(map.querySelectorAll(".parked-plan")).toHaveLength(0);
 fireEvent.keyDown(document.body,{key:"z",ctrlKey:true});expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
 fireEvent.keyDown(document.body,{key:"z",ctrlKey:true,shiftKey:true});expect(map.querySelectorAll(".draft-anchor")).toHaveLength(3);
});

test("automatic drawing dragging at the endpoint never finishes or adds an anchor",async()=>{
 const map=await start();
 fireEvent.click(map,{clientX:180,clientY:130,detail:1});fireEvent.click(map,{clientX:300,clientY:170,detail:1});
 const pointer=(type:string,x:number,y:number)=>{const e=new MouseEvent(type,{bubbles:true,clientX:x,clientY:y,button:0});Object.defineProperty(e,"pointerId",{value:1});fireEvent(map,e);};
 pointer("pointerdown",300,170);pointer("pointermove",315,170);pointer("pointerup",315,170);fireEvent.click(map,{clientX:315,clientY:170,detail:1});
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(2);
 pointer("pointerdown",390,190);pointer("pointerup",390,190);fireEvent.click(map,{clientX:390,clientY:190,detail:1});
 expect(map.querySelectorAll(".draft-anchor")).toHaveLength(3);expect(map.querySelectorAll(".parked-plan")).toHaveLength(0);
});

 test("construction omits redundant controls without removing keyboard editing",async()=>{await start();expect(screen.queryByRole("button",{name:"1点戻す"})).toBeNull();expect(screen.queryByRole("button",{name:"やり直す"})).toBeNull();expect(document.querySelector(".reference-tools")?.hasAttribute("hidden")).toBe(true);expect(document.querySelector(".track-editor")?.textContent).not.toContain("運行経路用：駅間接続");});

test("station placement hides management controls while retaining management access",async()=>{await start(placed);fireEvent.click(screen.getByRole("tab",{name:"建設"}));fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));expect(screen.queryByRole("region",{name:"駅名の自動設定"})).toBeNull();expect(screen.queryByRole("button",{name:"指定座標に配置"})).toBeNull();expect(screen.queryByRole("button",{name:"駅設置を取り消す"})).toBeNull();expect(screen.queryByRole("button",{name:"駅設置をやり直す"})).toBeNull();expect(screen.queryByRole("heading",{name:"配置済みの駅"})).toBeNull();fireEvent.click(screen.getByRole("tab",{name:"駅"}));expect(screen.getByRole("heading",{name:"配置済みの駅"})).toBeTruthy();expect(screen.getByRole("button",{name:"指定座標に配置"})).toBeTruthy();});
