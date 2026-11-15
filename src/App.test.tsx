// @vitest-environment jsdom
import React from "react";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { App } from "./App";
import { project, GSI_TILES, GSI_ATTRIBUTION_URL, DEFAULT_CAMERA, panCamera, visibleTiles, positionTile } from "./map";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));

const empty = { next_id: "3", players: [{ id: "1", name: "Player" }], companies: [{ id: "2", owner: "1", name: "Railway" }], stations: [], routes: [], route_paths: [], services: [], service_paths: [] };
const populated = { ...empty, next_id: "4", stations: [{ id: "3", owner: "2", name: "中央", latitude: 35.675, longitude: 139.75 }] };
// Keep semantic role/name queries inside their owning UI region. The app keeps
// inactive panels mounted, so document-wide a11y queries scan unrelated controls.
function region(selector: string) {
  const element = document.querySelector<HTMLElement>(selector);
  if (!element) throw new Error(`Missing test region: ${selector}`);
  return within(element);
}
function renderStationEditor() { const result = render(<App />); fireEvent.click(region(".background-choices").getByRole("button",{name:"自作サンプル（オフライン）"})); fireEvent.click(screen.getByRole("tab",{name:"建設"}));fireEvent.click(region(".editor-tabs").getByRole("tab", { name: "駅設置" })); return result; }
function desktop() {
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "engine_info") return { name: "Rosenfu", version: "test-version" };
    if (command === "world_snapshot") return empty;
    if (command === "add_station" || command === "load_world") return populated;
    if (command === "save_world") return { saved: true };
    throw new Error(`unexpected command: ${command}`);
  });
}
afterEach(() => { localStorage.removeItem("rosenfu.display-settings"); cleanup(); vi.useRealTimers(); vi.resetAllMocks(); });

test("browser preview is explicit and never invokes engine commands", () => {
  vi.mocked(isTauri).mockReturnValue(false);
  renderStationEditor();
  expect(screen.getByText(/ブラウザでの画面確認/)).toBeTruthy();
  expect(document.querySelector(".app-header .brand")).toBeNull();
  expect(document.querySelector(".app-header")?.textContent).not.toContain("Rosenfu");
  expect(screen.getByText("自作サンプル背景 / 実地図ではありません")).toBeTruthy();
  expect((screen.getByRole("button", { name: "保存" }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole("img", { name: "駅配置用サンプル地図" }));
  expect(invoke).not.toHaveBeenCalled();
});

test("desktop loads authoritative snapshot and engine version", async () => {
  desktop(); renderStationEditor();
  expect(await screen.findByText(/test-version/)).toBeTruthy();
  expect(invoke).toHaveBeenCalledWith("engine_info");
  expect(invoke).toHaveBeenCalledWith("world_snapshot");
  expect((screen.getByRole("button", { name: "保存" }) as HTMLButtonElement).disabled).toBe(false);
});

test("startup failure supports retry", async () => {
  desktop(); vi.mocked(invoke).mockRejectedValueOnce(new Error("offline"));
  renderStationEditor();
  fireEvent.click(await screen.findByRole("button", { name: "再試行" }));
  expect(await screen.findByText(/test-version/)).toBeTruthy();
  expect(screen.queryByRole("alert")).toBeNull();
});

test("station map placement requires a previously built alignment", async () => {
  desktop(); renderStationEditor(); await screen.findByText(/test-version/);
  fireEvent.click(screen.getByRole("button", { name: "駅配置" }));
  const map = interactiveMap();
  fireEvent.click(map, { clientX: 250, clientY: 150 });
  expect(await screen.findByText(/処理できませんでした: 駅は線路上に置いてください/)).toBeTruthy();
  expect(invoke).not.toHaveBeenCalledWith("add_station", expect.anything());
});

test.each([false,true])("coordinate entry preserves state on add_station failure (interleaved autosave_status=%s)", async (interleaveStatus) => {
  desktop(); renderStationEditor(); await screen.findByText(/test-version/);
  const original=vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command,args)=>command==="add_station"?Promise.reject(new Error("invalid station")):original(command,args));
  // An unrelated failing read must not consume the station failure fixture.
  if(interleaveStatus)await expect(invoke("autosave_status")).rejects.toThrow("unexpected command: autosave_status");
  fireEvent.click(screen.getByRole("tab", {name:"駅"}));fireEvent.click(screen.getByRole("button", { name: "指定座標に配置" }));
  expect((await screen.findByRole("alert")).querySelector("span")).toHaveProperty("textContent", "処理できませんでした: invalid station");
  expect(screen.queryByText("未保存の変更あり")).toBeNull();
  expect(screen.getByText(/まだ駅はありません/)).toBeTruthy();
});

test("save and load errors retain the current snapshot", async () => {
  desktop(); renderStationEditor(); await screen.findByText(/test-version/);
  vi.mocked(invoke).mockRejectedValueOnce(new Error("save denied"));
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await screen.findByText(/save denied/);
  expect(screen.queryByText("ワールドと未着工計画を保存しました。")).toBeNull();
  vi.mocked(invoke).mockRejectedValueOnce(new Error("corrupt file"));
  fireEvent.click(screen.getByRole("button", { name: "読込" }));
  await screen.findByText(/corrupt file/);
  expect(screen.getByText(/まだ駅はありません/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "読込" }));
  await screen.findByText("保存したワールドを読み込みました。");
  expect(screen.getByRole("button", { name: /中央 35.67500/, hidden:true })).toBeTruthy();
});

test("loading over unsaved station edits requires an explicit choice", async () => {
  desktop(); renderStationEditor(); await screen.findByText(/test-version/);
  fireEvent.click(screen.getByRole("tab", {name:"駅"}));fireEvent.click(screen.getByRole("button", { name: "指定座標に配置" }));
  await screen.findByText("新駅を配置しました。");
  fireEvent.click(screen.getByRole("button", { name: "読込" }));
  expect(invoke).not.toHaveBeenCalledWith("load_world");
  fireEvent.click(screen.getByRole("button", { name: "戻る" }));
  expect(invoke).not.toHaveBeenCalledWith("load_world");
  fireEvent.click(screen.getByRole("button", { name: "読込" }));
  fireEvent.click(screen.getByRole("button", { name: "読込を続ける" }));
  await screen.findByText("保存したワールドを読み込みました。");
  expect(screen.queryByText("未保存の変更あり")).toBeNull();
});

test("pending operations block duplicate actions and successful save clears dirty state", async () => {
  desktop(); renderStationEditor(); await screen.findByText(/test-version/);
  fireEvent.click(screen.getByRole("tab", {name:"駅"}));fireEvent.click(screen.getByRole("button", { name: "指定座標に配置" }));
  await screen.findByText("未保存の変更あり");
  let finish!: (value: { saved: boolean }) => void;
  vi.mocked(invoke).mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  const save = screen.getByRole("button", { name: "保存" });
  fireEvent.click(save); fireEvent.click(save);
  expect((save as HTMLButtonElement).disabled).toBe(true);
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "save_world")).toHaveLength(1);
  finish({ saved: true });
  await waitFor(() => expect(screen.queryByText("未保存の変更あり")).toBeNull());
  expect(screen.getByText("ワールドと未着工計画を保存しました。")).toBeTruthy();
});

const twoStations = {
  ...populated, next_id: "5",
  stations: [...populated.stations, { id: "4", owner: "2", name: "東", latitude: 35.70, longitude: 139.80 }],
};
const connected = {...twoStations,next_id:"6",alignments:[{id:"5",owner:"2",name:"接続線形",track_type:"normal",points:twoStations.stations.map(({latitude,longitude})=>({latitude,longitude}))}],station_placements:twoStations.stations.map((station,i)=>({station_id:station.id,alignment_id:"5",position:i,length_m:100,width_m:12}))};
async function renderWithStations(snapshot: unknown = twoStations) {
  desktop();
  vi.mocked(invoke).mockResolvedValueOnce({ name: "Rosenfu", version: "test-version" }).mockResolvedValueOnce(snapshot);
  renderStationEditor(); await screen.findByText(/test-version/);
  fireEvent.click(screen.getByRole("tab", { name: "路線" }));
}






const network = {
  ...connected, next_id: "12",
  stations: [...twoStations.stations,
    { id: "7", owner: "2", name: "北", latitude: 35.72, longitude: 139.82 },
    { id: "8", owner: "2", name: "離島A", latitude: 35.62, longitude: 139.67 },
    { id: "9", owner: "2", name: "離島B", latitude: 35.63, longitude: 139.68 },
  ],
  routes: [{ id: "11", owner: "2", name: "既存線", start: "3" }],
  route_paths:[{route_id:"11",segments:[{kind:"alignment",alignment_id:"5",from:"3",to:"4"}],member_stations:["3","4"]}],
};






test("company name is applied to the existing company and unsubmitted input is not saved", async () => {
  await renderWithStations(network); fireEvent.click(screen.getByRole("tab", { name: "設定" }));
  fireEvent.change(screen.getByLabelText("会社名"), { target: { value: "  新しい鉄道  " } });
  expect((screen.getByRole("button", { name: "保存" }) as HTMLButtonElement).disabled).toBe(true);
  vi.mocked(invoke).mockRejectedValueOnce(new Error("rename rejected"));
  fireEvent.click(screen.getByRole("button", { name: "設定変更" }));
  await screen.findByText(/rename rejected/);
  expect((screen.getByLabelText("会社名") as HTMLInputElement).value).toBe("  新しい鉄道  ");
  vi.mocked(invoke).mockResolvedValueOnce({ ...network, companies: [{ ...network.companies[0], name: "新しい鉄道" }] });
  fireEvent.click(screen.getByRole("button", { name: "設定変更" }));
  await screen.findByText("会社名を変更しました。");
  expect(invoke).toHaveBeenCalledWith("rename_company", { name: "新しい鉄道" });
  expect((screen.getByLabelText("会社名") as HTMLInputElement).value).toBe("新しい鉄道");
  expect((screen.getByRole("button", { name: "保存" }) as HTMLButtonElement).disabled).toBe(false);
});


test("map station placement is disabled outside the station editor tab", async () => {
  desktop(); renderStationEditor(); await screen.findByText(/test-version/);
  const map = screen.getByRole("img", { name: "駅配置用サンプル地図" });
  map.getBoundingClientRect = () => ({ x: 0, y: 0, left: 0, top: 0, right: 500, bottom: 300, width: 500, height: 300, toJSON: () => ({}) });
  for (const tab of ["線路", "路線", "系統", "設定"]) {
    fireEvent.click(screen.getByRole("tab", { name: tab }));
    fireEvent.click(map, { clientX: 250, clientY: 150 });
    expect(map.classList.contains("placement-enabled")).toBe(tab === "線路");
    if (tab !== "線路") expect(screen.getByText("ドラッグで移動 · ホイールで拡大縮小")).toBeTruthy();
  }
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "add_station")).toHaveLength(0);
  fireEvent.click(screen.getByRole("tab",{name:"建設"}));fireEvent.click(screen.getByRole("tab", { name: "駅設置" }));
  fireEvent.click(screen.getByRole("button", { name: "駅配置" }));
  expect(map.classList.contains("placement-enabled")).toBe(true);
  expect(screen.getByText("線路にカーソルを重ねてクリックで駅を配置")).toBeTruthy();
  fireEvent.click(map, { clientX: 250, clientY: 150 });
  await screen.findByText(/処理できませんでした: 駅は線路上に置いてください/);
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "add_station")).toHaveLength(0);
});

test("online map requests only fixed tile images after explicit opt-in and shows attribution", async () => {
  vi.mocked(isTauri).mockReturnValue(false); renderStationEditor();
  const sample = screen.getByRole("img", { name: "駅配置用サンプル地図" });
  expect(sample.querySelectorAll("image")).toHaveLength(0);
  fireEvent.click(screen.getByRole("button", { name: "地理院地図（オンライン）" }));
  const online = screen.getByRole("img", { name: "駅配置用地理院地図" });
  const images = Array.from(online.querySelectorAll("image"));
  expect(images).toHaveLength(GSI_TILES.length);
  expect(images.map((image) => image.getAttribute("href"))).toEqual(GSI_TILES.map((tile) => tile.url));
  expect(online.getAttribute("preserveAspectRatio")).toBe("xMidYMid slice");
  expect(online.querySelector('[clip-path="url(#map-extent)"]')).toBeTruthy();
  expect(screen.getByRole("link", { name: "出典: 国土地理院 地理院タイル（淡色地図）" }).getAttribute("href")).toBe(GSI_ATTRIBUTION_URL);
  expect(screen.getByText(/架空の鉄道設備を重畳表示。背景地図とは別データです/)).toBeTruthy();
  expect(screen.getByText(/地理院タイルを読み込み中/)).toBeTruthy();
  for (const image of images) fireEvent.load(image);
  expect(await screen.findByText(`地理院タイルを表示中（${GSI_TILES.length}枚）`)).toBeTruthy();
  expect(invoke).not.toHaveBeenCalled();
});

test("tile failure remains explicit without automatic retry or fallback and sample return removes network images", async () => {
  vi.mocked(isTauri).mockReturnValue(false); renderStationEditor();
  fireEvent.click(screen.getByRole("button", { name: "地理院地図（オンライン）" }));
  const map = screen.getByRole("img", { name: "駅配置用地理院地図" });
  const images = Array.from(map.querySelectorAll("image"));
  fireEvent.error(images[0]);
  for (const image of images.slice(1)) fireEvent.load(image);
  expect(await screen.findByText("地図の一部を表示できません。")).toBeTruthy();
  expect(screen.getByRole("alert").textContent).toContain("1枚の読み込みに失敗");
  expect(map.querySelectorAll("image")).toHaveLength(GSI_TILES.length - 1);
  expect(map.querySelectorAll("[data-failed-tile]")).toHaveLength(1);
  expect(screen.getByRole("button", { name: "地理院地図（オンライン）" }).getAttribute("aria-pressed")).toBe("true");
  fireEvent.click(screen.getByRole("button", { name: "サンプル背景に戻す" }));
  expect(screen.getByRole("img", { name: "駅配置用サンプル地図" }).querySelectorAll("image")).toHaveLength(0);
  expect(screen.queryByText(/出典: 国土地理院/)).toBeNull();
  expect(invoke).not.toHaveBeenCalled();
});

test("background switching is display-only and outside viewport clicks never create stations", async () => {
  desktop(); renderStationEditor(); await screen.findByText(/test-version/);
  await waitFor(()=>expect(screen.queryByText("ワールドを更新しています。")).toBeNull());
  const previousCalls = vi.mocked(invoke).mock.calls.length;
  fireEvent.click(screen.getByRole("button", { name: "地理院地図（オンライン）" }));
  fireEvent.click(screen.getByRole("button", { name: "自作サンプル（オフライン）" }));
  expect(vi.mocked(invoke).mock.calls).toHaveLength(previousCalls);
  const map = screen.getByRole("img", { name: "駅配置用サンプル地図" });
  map.getBoundingClientRect = () => ({ x: 0, y: 0, left: 0, top: 0, right: 1000, bottom: 300, width: 1000, height: 300, toJSON: () => ({}) });
  fireEvent.click(screen.getByRole("button", { name: "駅配置" }));
  fireEvent.click(map, { clientX: -10, clientY: 150 });
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "add_station")).toHaveLength(0);
  expect(screen.queryByText("未保存の変更あり")).toBeNull();
});

test("rapid pan debounces new tile URLs while keeping shared DOM and existing tiles georeferenced", () => {
  vi.useFakeTimers(); vi.mocked(isTauri).mockReturnValue(false); renderStationEditor();
  fireEvent.click(screen.getByLabelText("駅名を表示"));
  fireEvent.click(screen.getByRole("button", { name: "地理院地図（オンライン）" }));
  const map = screen.getByRole("img", { name: "駅配置用地理院地図" });
  const original = new Map(Array.from(map.querySelectorAll("image")).map(image=>[image.getAttribute("data-tile-id")!,image]));
  for(const image of original.values()) fireEvent.load(image);
  fireEvent.click(screen.getByRole("button", { name: "東へ移動" }));
  act(()=>vi.advanceTimersByTime(100));
  fireEvent.click(screen.getByRole("button", { name: "東へ移動" }));
  const camera = panCamera(panCamera(DEFAULT_CAMERA,300,0),300,0);
  expect(Array.from(map.querySelectorAll("image")).map(image=>image.getAttribute("data-tile-id"))).toEqual(Array.from(original.keys()));
  const first = GSI_TILES[0];
  expect(Number(original.get(first.id)!.getAttribute("x"))).toBeCloseTo(positionTile(first,camera).x,8);
  const marker = map.querySelector(".station-marker")!;
  const station = project(35.685,139.735,camera);
  expect(Number(marker.querySelector("text")?.getAttribute("x"))).toBeCloseTo(station.x + 18);
  act(()=>vi.advanceTimersByTime(249));
  expect(Array.from(map.querySelectorAll("image")).map(image=>image.getAttribute("data-tile-id"))).toEqual(Array.from(original.keys()));
  act(()=>vi.advanceTimersByTime(1));
  const current = Array.from(map.querySelectorAll("image:not([data-fallback-tile])"));
  expect(current.map(image=>image.getAttribute("href"))).toEqual(visibleTiles(camera).map(tile=>tile.url));
  const shared = current.filter(image=>original.has(image.getAttribute("data-tile-id")!));
  expect(shared.length).toBeGreaterThan(0);
  for(const image of shared) expect(image).toBe(original.get(image.getAttribute("data-tile-id")!));
  expect(invoke).not.toHaveBeenCalled();
});

test("tile result metadata is discarded when tiles leave the viewport and zoom remains bounded", () => {
  vi.useFakeTimers(); vi.mocked(isTauri).mockReturnValue(false); renderStationEditor();
  fireEvent.click(region(".background-choices").getByRole("button", { name: "地理院地図（オンライン）" }));
  const map = screen.getByRole("img", { name: "駅配置用地理院地図" });
  const first = map.querySelector("image")!; const id = first.getAttribute("data-tile-id")!;
  fireEvent.error(first);
  expect(map.querySelector(`[data-failed-tile="${id}"]`)).toBeTruthy();
  for(let i=0;i<6;i++) fireEvent.click(region(".camera-controls").getByRole("button", { name: "東へ移動" }));
  act(()=>vi.advanceTimersByTime(250));
  expect(map.querySelector(`[data-failed-tile="${id}"]`)).toBeNull();
  fireEvent.click(region(".camera-controls").getByRole("button", { name: "東京へ戻る" }));
  act(()=>vi.advanceTimersByTime(250));
  expect(map.querySelector(`image[data-tile-id="${id}"]`)).toBeTruthy();
  expect((region(".camera-controls").getByRole("button", { name: "縮小" }) as HTMLButtonElement).disabled).toBe(false);
  for(let i=0;i<10;i++) fireEvent.click(region(".camera-controls").getByRole("button", { name: "拡大" }));
  act(()=>vi.advanceTimersByTime(200));
  expect((region(".camera-controls").getByRole("button", { name: "拡大" }) as HTMLButtonElement).disabled).toBe(true);
  act(()=>vi.advanceTimersByTime(250));
  expect(Array.from(map.querySelectorAll("image")).every(image=>image.getAttribute("href")?.includes("/pale/17/"))).toBe(true);
  expect(invoke).not.toHaveBeenCalled();
});

test("camera changes do not edit world IDs/coordinates and legacy reload restores default view", async () => {
  desktop(); renderStationEditor(); await screen.findByText(/test-version/);
  const originalSnapshot = JSON.stringify(populated);
  fireEvent.click(screen.getByRole("tab", {name:"駅"}));fireEvent.click(screen.getByRole("button", { name: "指定座標に配置" }));
  await screen.findByText("新駅を配置しました。");
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await screen.findByText("ワールドと未着工計画を保存しました。");
  const count = vi.mocked(invoke).mock.calls.length;
  fireEvent.change(screen.getByLabelText("中心緯度"), { target: { value: "43.06" } });
  fireEvent.change(screen.getByLabelText("中心経度"), { target: { value: "141.35" } });
  fireEvent.click(screen.getByRole("button", { name: "座標へ移動" }));
  fireEvent.click(screen.getByRole("button", { name: "拡大" }));
  expect(vi.mocked(invoke).mock.calls).toHaveLength(count);
  expect(screen.queryByText("未保存の変更あり")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "読込" }));
  fireEvent.click(screen.getByRole("button", { name: "読込を続ける" }));
  await screen.findByText("保存したワールドを読み込みました。");
  expect((screen.getByLabelText("中心緯度") as HTMLInputElement).value).toBe(DEFAULT_CAMERA.latitude.toFixed(5));
  expect((screen.getByLabelText("中心経度") as HTMLInputElement).value).toBe(DEFAULT_CAMERA.longitude.toFixed(5));
  expect(screen.getByRole("button", { name: /中央 35.67500, 139.75000/ })).toBeTruthy();
  expect(JSON.stringify(populated)).toBe(originalSnapshot);
  expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==="add_station")).toHaveLength(1);
});

function pointer(target: Element, type: string, x: number, y: number, id = 1) {
  const event = new Event(type, { bubbles: true, cancelable: true });
  Object.assign(event, { clientX:x, clientY:y, pointerId:id, button:0, isPrimary:true });
  fireEvent(target,event);
}
function interactiveMap() {
  const map = screen.getByRole("img", { name: "駅配置用サンプル地図" });
  map.getBoundingClientRect = () => ({ x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({}) });
  const captures = new Set<number>();
  Object.assign(map, { setPointerCapture: vi.fn((id:number)=>captures.add(id)), hasPointerCapture:(id:number)=>captures.has(id), releasePointerCapture:vi.fn((id:number)=>captures.delete(id)) });
  return map;
}

test("move is the default mode and drags never create stations even in placement mode", async () => {
  desktop(); renderStationEditor(); await screen.findByText(/test-version/);
  const map = interactiveMap();
  fireEvent.click(map,{clientX:250,clientY:150});
  expect(invoke).not.toHaveBeenCalledWith("add_station",expect.anything());
  fireEvent.click(screen.getByRole("button",{name:"駅配置"}));
  pointer(map,"pointerdown",250,150); pointer(map,"pointermove",270,150); pointer(map,"pointerup",270,150);
  fireEvent.click(map,{clientX:270,clientY:150});
  expect(invoke).not.toHaveBeenCalledWith("add_station",expect.anything());
  expect(Number((screen.getByLabelText("中心経度") as HTMLInputElement).value)).toBeLessThan(DEFAULT_CAMERA.longitude);
  pointer(map,"pointerdown",250,150); pointer(map,"pointermove",252,150); pointer(map,"pointerup",252,150);
  fireEvent.click(map,{clientX:252,clientY:150});
  await screen.findByText(/処理できませんでした: 駅は線路上に置いてください/);
  expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==="add_station")).toHaveLength(0);
});

test("pointer cancel, lost capture, and outside release suppress construction", async () => {
  desktop(); renderStationEditor(); await screen.findByText(/test-version/);
  const map = interactiveMap(); fireEvent.click(screen.getByRole("button",{name:"駅配置"}));
  for(const end of ["pointercancel","lostpointercapture"]) {
    pointer(map,"pointerdown",250,150); pointer(map,end,250,150); fireEvent.click(map,{clientX:250,clientY:150});
  }
  pointer(map,"pointerdown",250,150); pointer(map,"pointermove",270,150); pointer(map,"pointerup",700,450); fireEvent.click(map,{clientX:250,clientY:150});
  expect(invoke).not.toHaveBeenCalledWith("add_station",expect.anything());
  expect((map as unknown as {releasePointerCapture:ReturnType<typeof vi.fn>}).releasePointerCapture).toHaveBeenCalled();
});

test("tool and mode switches cancel active drags and require renewed placement intent", async () => {
  desktop(); renderStationEditor(); await screen.findByText(/test-version/);
  const map = interactiveMap(); fireEvent.click(region(".camera-controls").getByRole("button",{name:"駅配置"}));
  pointer(map,"pointerdown",250,150); pointer(map,"pointermove",270,150);
  fireEvent.click(screen.getByRole("tab",{name:"路線"}));
  const longitude = (region(".camera-controls").getByLabelText("中心経度") as HTMLInputElement).value;
  pointer(map,"pointermove",400,150); pointer(map,"pointerup",400,150);
  fireEvent.click(screen.getByRole("tab",{name:"建設"}));fireEvent.click(region(".editor-tabs").getByRole("tab",{name:"駅設置"}));
  fireEvent.click(map,{clientX:250,clientY:150});
  expect((region(".camera-controls").getByLabelText("中心経度") as HTMLInputElement).value).toBe(longitude);
  expect(region(".camera-controls").getByRole("button",{name:"駅配置"}).getAttribute("aria-pressed")).toBe("true");
  fireEvent.click(region(".camera-controls").getByRole("button",{name:"駅配置"}));
  pointer(map,"pointerdown",250,150); fireEvent.click(region(".camera-controls").getByRole("button",{name:"移動"})); pointer(map,"pointerup",250,150); fireEvent.click(map,{clientX:250,clientY:150});
  expect(invoke).not.toHaveBeenCalledWith("add_station",expect.anything());
});

test("wheel zoom is map-only, accumulated, vertical-only, and leaves ctrl-wheel untouched", () => {
  vi.useFakeTimers(); vi.mocked(isTauri).mockReturnValue(false); renderStationEditor();
  const map = interactiveMap();
  const wheel = (target:Element, options:WheelEventInit) => {
    const event = new WheelEvent("wheel", {bubbles:true,cancelable:true,...options});
    fireEvent(target,event); return event;
  };
  expect(wheel(document.body,{deltaY:-10}).defaultPrevented).toBe(false);
  expect(wheel(map,{deltaY:-10,ctrlKey:true}).defaultPrevented).toBe(false);
  expect(wheel(map,{deltaX:100,deltaY:1}).defaultPrevented).toBe(false);
  expect(screen.getByText("ZL 12 / Web Mercator")).toBeTruthy();
  expect(wheel(map,{deltaY:-100,clientX:250,clientY:150}).defaultPrevented).toBe(true);
  for(let i=0;i<3;i++) wheel(map,{deltaY:-100,clientX:250,clientY:150});
  expect(screen.getByText("ZL 12 / Web Mercator")).toBeTruthy();
  act(()=>vi.advanceTimersByTime(180));
  expect(screen.getByText("ZL 13 / Web Mercator")).toBeTruthy();
  wheel(map,{deltaY:400,clientX:250,clientY:150});
  wheel(map,{deltaY:100,clientX:250,clientY:150});
  act(()=>vi.advanceTimersByTime(180));
  expect(screen.getByText("ZL 12 / Web Mercator")).toBeTruthy();
  expect(invoke).not.toHaveBeenCalled();
});

test("existing equipment short clicks avoid capture and retargeted clicks cannot create duplicate stations", async () => {
  await renderWithStations(connected); fireEvent.click(screen.getByRole("tab",{name:"建設"}));fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));
  const map = interactiveMap(); fireEvent.click(screen.getByRole("button",{name:"駅配置"}));
  const marker = map.querySelector(".station-marker")!;
  pointer(marker,"pointerdown",250,150); pointer(marker,"pointerup",250,150);
  expect((map as unknown as {setPointerCapture:ReturnType<typeof vi.fn>}).setPointerCapture).not.toHaveBeenCalled();
  fireEvent.click(marker);
  expect(marker.classList.contains("selected")).toBe(true);
  fireEvent.click(map,{clientX:250,clientY:150});
  const line = map.querySelector(".alignment-hit")!;
  pointer(line,"pointerdown",260,150); pointer(line,"pointerup",260,150); fireEvent.click(map,{clientX:260,clientY:150});
  expect(invoke).not.toHaveBeenCalledWith("add_station",expect.anything());
});

test("changing from move to placement midgesture cannot turn its eventual click into construction", async () => {
  desktop(); renderStationEditor(); await screen.findByText(/test-version/);
  const map = interactiveMap();
  pointer(map,"pointerdown",250,150);
  fireEvent.click(screen.getByRole("button",{name:"駅配置"}));
  pointer(map,"pointermove",251,150); pointer(map,"pointerup",251,150); fireEvent.click(map,{clientX:251,clientY:150});
  expect(invoke).not.toHaveBeenCalledWith("add_station",expect.anything());
  pointer(map,"pointerdown",250,150); pointer(map,"pointerup",250,150); fireEvent.click(map,{clientX:250,clientY:150});
  await screen.findByText(/処理できませんでした: 駅は線路上に置いてください/);
  expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==="add_station")).toHaveLength(0);
});

test("leaving before drag threshold cancels and wheel cannot interrupt an active drag", async () => {
  desktop(); renderStationEditor(); await screen.findByText(/test-version/);
  const map = interactiveMap(); fireEvent.click(screen.getByRole("button",{name:"駅配置"}));
  pointer(map,"pointerdown",1,150); pointer(map,"pointerout",-1,150); pointer(map,"pointerup",250,150); fireEvent.click(map,{clientX:250,clientY:150});
  expect(invoke).not.toHaveBeenCalledWith("add_station",expect.anything());
  pointer(map,"pointerdown",250,150); pointer(map,"pointermove",270,150);
  fireEvent(map,new WheelEvent("wheel",{bubbles:true,cancelable:true,deltaY:-100}));
  expect(screen.getByText("ZL 12 / Web Mercator")).toBeTruthy();
  pointer(map,"pointerup",270,150); fireEvent.click(map,{clientX:270,clientY:150});
  expect(invoke).not.toHaveBeenCalledWith("add_station",expect.anything());
});

test("bubbled child capture loss does not cancel the active SVG drag", async () => {
  desktop(); renderStationEditor(); await screen.findByText(/test-version/);
  const map = interactiveMap();
  const background = map.querySelector("rect")!;
  pointer(background,"pointerdown",250,150);
  pointer(map,"pointermove",270,150);
  const firstLongitude = Number((screen.getByLabelText("中心経度") as HTMLInputElement).value);
  pointer(background,"lostpointercapture",270,150);
  pointer(map,"pointermove",290,150);
  const secondLongitude = Number((screen.getByLabelText("中心経度") as HTMLInputElement).value);
  expect(secondLongitude).toBeLessThan(firstLongitude);
  expect(map.classList.contains("map-dragging")).toBe(true);
  pointer(map,"pointerup",290,150);
  fireEvent.click(map,{clientX:290,clientY:150});
  expect(invoke).not.toHaveBeenCalledWith("add_station",expect.anything());
  expect(map.classList.contains("map-dragging")).toBe(false);
});

const serviceNetwork = {
  ...network, next_id: "13",
  services: [{ id: "12", owner: "2", name: "折返し系統", start: "3" }],
  service_paths:[{service_id:"12",next_leg_key:"3",legs:[{key:"1",segment:{kind:"alignment",alignment_id:"5",from:"3",to:"4"}},{key:"2",segment:{kind:"alignment",alignment_id:"5",from:"4",to:"3"}}],stops:[{kind:"start"},{kind:"on_leg",leg_key:"1",station_id:"4"},{kind:"on_leg",leg_key:"2",station_id:"3"}]}],
};







test("browser preview includes a read-only service without implying train operation", () => {
  vi.mocked(isTauri).mockReturnValue(false); renderStationEditor();
  fireEvent.click(screen.getByRole("tab", { name: "系統" }));
  expect((region("#workspace-panel-services").getByRole("button", { name: "系統編集: サンプル系統" }) as HTMLButtonElement).disabled).toBe(true);
  expect(region("#workspace-panel-services").getByText(/閲覧専用のサンプルです。種別・ダイヤ・列車運行は未実装です/)).toBeTruthy();
  expect(invoke).not.toHaveBeenCalled();
});

async function editEquipment(snapshot: unknown, action: (command: string, args: any) => Promise<unknown>) {
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === "engine_info") return { name: "Rosenfu", version: "test-version" };
    if (command === "world_snapshot") return snapshot;
    return action(command, args);
  });
  renderStationEditor(); await screen.findByText(/test-version/);
}
function selectFirstStation() { fireEvent.click(document.querySelector(".station-entry")!); }

test("station rename keeps draft on failure and blocks save/switch/load until explicit discard", async () => {
  await editEquipment(twoStations, async () => { throw new Error("rename blocked"); });
  selectFirstStation();
  fireEvent.change(region(".equipment-editor").getByLabelText("選択駅の名前"), { target: { value: "変更案" } });
  expect((region(".save-controls").getByRole("button", { name: "保存" }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(region(".equipment-editor").getByRole("button", { name: "駅名を適用" }));
  await screen.findByText(/rename blocked/);
  expect((region(".equipment-editor").getByLabelText("選択駅の名前") as HTMLInputElement).value).toBe("変更案");
  fireEvent.click(document.querySelectorAll(".station-entry")[1]);
  fireEvent.click(region(".equipment-editor + .inline-confirm").getByRole("button", { name: "駅名編集を続ける" }));
  expect((region(".equipment-editor").getByLabelText("選択駅の名前") as HTMLInputElement).value).toBe("変更案");
  fireEvent.click(region(".save-controls").getByRole("button", { name: "読込" }));
  expect(region(".load-confirm").getByRole("button", { name: "読込を続ける" })).toBeTruthy();
  expect(invoke).not.toHaveBeenCalledWith("load_world");
  fireEvent.click(region(".load-confirm").getByRole("button", { name: "戻る" }));
  fireEvent.click(document.querySelectorAll(".station-entry")[1]);
  fireEvent.click(region(".equipment-editor + .inline-confirm").getByRole("button", { name: "駅名下書きを破棄して切替" }));
  expect((region(".equipment-editor").getByLabelText("選択駅の名前") as HTMLInputElement).value).toBe(twoStations.stations[1].name);
});

test("station rename submits once, retains ID and coordinates, and needs world save", async () => {
  let finish!: (value: unknown) => void;
  await editEquipment(populated, async () => new Promise((resolve) => { finish = resolve; }));
  selectFirstStation();
  fireEvent.change(screen.getByLabelText("選択駅の名前"), { target: { value: " 新中央 " } });
  const apply = screen.getByRole("button", { name: "駅名を適用" });
  fireEvent.click(apply); fireEvent.click(apply);
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "rename_station")).toHaveLength(1);
  expect(invoke).toHaveBeenCalledWith("rename_station", { stationId: "3", name: "新中央" });
  await act(async () => finish({ ...populated, stations: [{ ...populated.stations[0], name: "新中央" }] }));
  expect((screen.getByLabelText("選択駅の名前") as HTMLInputElement).value).toBe("新中央");
  expect(screen.getByText("未保存の変更あり")).toBeTruthy();
  expect(document.querySelector(".station-entry small")?.textContent).toBe("35.67500, 139.75000");
});

test("isolated station deletion confirms, handles failure, and prevents duplicate requests", async () => {
  let fail = true;
  let finish!: (value: unknown) => void;
  await editEquipment(populated, async () => {
    if (fail) throw new Error("delete blocked");
    return new Promise((resolve) => { finish = resolve; });
  });
  selectFirstStation();
  fireEvent.click(screen.getByRole("button", { name: "選択駅を削除" }));
  expect(invoke).not.toHaveBeenCalledWith("delete_station", expect.anything());
  fireEvent.click(screen.getByRole("button", { name: "削除をやめる" }));
  expect(document.querySelectorAll(".station-entry")).toHaveLength(1);
  fireEvent.click(screen.getByRole("button", { name: "選択駅を削除" }));
  fireEvent.click(screen.getByRole("button", { name: "削除を確定" }));
  await screen.findByText(/delete blocked/);
  expect(document.querySelectorAll(".station-entry")).toHaveLength(1);
  fail = false;
  const confirm = screen.getByRole("button", { name: "削除を確定" });
  fireEvent.click(confirm); fireEvent.click(confirm);
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "delete_station")).toHaveLength(2);
  await act(async () => finish({ ...empty, next_id: populated.next_id }));
  expect(document.querySelectorAll(".station-entry")).toHaveLength(0);
  expect(screen.queryByLabelText("選択駅の名前")).toBeNull();
  expect(screen.getByText("未保存の変更あり")).toBeTruthy();
});


test("route and service references are explained and browser equipment editing stays disabled", async () => {
  await editEquipment(serviceNetwork, async () => { throw new Error("must not invoke"); });
  selectFirstStation();
  expect(screen.getByText(/削除不可:.*系統: 折返し系統/)).toBeTruthy();
  cleanup(); vi.mocked(invoke).mockClear(); vi.mocked(isTauri).mockReturnValue(false);
  renderStationEditor(); selectFirstStation();
  expect((screen.getByLabelText("選択駅の名前") as HTMLInputElement).closest("fieldset")?.disabled).toBe(true);
  expect((screen.getByRole("button", { name: "選択駅を削除" }) as HTMLButtonElement).disabled).toBe(true);
  expect(invoke).not.toHaveBeenCalled();
});

test("background choices avoid native selects and repeated clicks preserve selection", () => {
  vi.mocked(isTauri).mockReturnValue(false); render(<App />); fireEvent.click(screen.getByRole("button",{name:"自作サンプル（オフライン）"}));
  expect(document.querySelector("select#background-source")).toBeNull();
  const sample = screen.getByRole("button", {name:"自作サンプル（オフライン）"});
  const online = screen.getByRole("button", {name:"地理院地図（オンライン）"});
  expect(sample.getAttribute("aria-pressed")).toBe("true");
  fireEvent.click(sample); fireEvent.click(sample);
  expect(document.querySelectorAll("svg image")).toHaveLength(0);
  fireEvent.click(online);
  expect(online.getAttribute("aria-pressed")).toBe("true");
  expect(document.querySelectorAll("svg image").length).toBeGreaterThan(0);
  fireEvent.click(sample);
  expect(sample.getAttribute("aria-pressed")).toBe("true");
  expect(document.querySelectorAll("svg image")).toHaveLength(0);
  expect(invoke).not.toHaveBeenCalled();
});
test("offline scene and grid follow camera pan and zoom without network requests", async () => {
  vi.mocked(isTauri).mockReturnValue(false); render(<App />); fireEvent.click(screen.getByRole("button",{name:"自作サンプル（オフライン）"}));
  const scene = document.querySelector("[data-sample-scene]")!;
  expect(scene.getAttribute("patternTransform")).toBe("translate(0 0) scale(1)");
  fireEvent.click(screen.getByRole("button", {name:"東へ移動"}));
  const panned = scene.getAttribute("patternTransform")!;
  expect(panned).not.toBe("translate(0 0) scale(1)");
  expect(document.querySelector("#sample-grid")!.getAttribute("patternTransform")).toBe(panned);
  fireEvent.click(screen.getByRole("button", {name:"拡大"}));
  await waitFor(()=>expect(scene.getAttribute("patternTransform")).toContain("scale(2)"));
  expect(document.querySelectorAll("svg image")).toHaveLength(0);
  fireEvent.click(screen.getByRole("button", {name:"東京へ戻る"}));
  expect(scene.getAttribute("patternTransform")).toBe("translate(0 0) scale(1)");
  expect(invoke).not.toHaveBeenCalled();
});

test("offline background uses a continuous camera-anchored pattern and fills the window", () => {
  vi.mocked(isTauri).mockReturnValue(false); render(<App />); fireEvent.click(screen.getByRole("button",{name:"自作サンプル（オフライン）"}));
  const map=screen.getByRole("img",{name:"駅配置用サンプル地図"});
  expect(map.getAttribute("preserveAspectRatio")).toBe("xMidYMid slice");
  expect(map.querySelector("pattern#sample-scene")).toBeTruthy();
  expect(map.querySelector('.sample-background rect[fill="url(#sample-scene)"]')).toBeTruthy();
  for(let i=0;i<8;i++) fireEvent.click(screen.getByRole("button",{name:"東へ移動"}));
  expect(map.querySelector('pattern#sample-scene')?.getAttribute("patternTransform")).not.toBe("translate(0 0) scale(1)");
  expect(map.querySelectorAll("image")).toHaveLength(0);
});

test("OSM is the normal background with attribution and offline sample remains available",async()=>{
 vi.mocked(isTauri).mockReturnValue(false); render(<App />);
 const map=screen.getByRole("img",{name:"駅配置用OpenStreetMap"});
 expect(map.querySelectorAll("image").length).toBeGreaterThan(0);
 for(const image of map.querySelectorAll("image")) expect(image.getAttribute("href")).toMatch(/^https:\/\/tile\.openstreetmap\.org\//);
 expect(within(screen.getByLabelText("地図の出典")).getByRole("link",{name:"© OpenStreetMap contributors"}).getAttribute("href")).toBe("https://www.openstreetmap.org/copyright");
 fireEvent.error(map.querySelector("image")!);expect(await screen.findByText(/枚の読み込みに失敗/)).toBeTruthy();
 fireEvent.click(screen.getByRole("button",{name:"自作サンプル（オフライン）"}));
 expect(screen.getByRole("img",{name:"駅配置用サンプル地図"}).querySelectorAll("image")).toHaveLength(0);
 fireEvent.click(screen.getByRole("button",{name:"OpenStreetMap（オンライン）"}));
 expect(screen.getByRole("img",{name:"駅配置用OpenStreetMap"}).querySelectorAll("[data-failed-tile]")).toHaveLength(0);
});

test("persisted alignment route is shown and protected against legacy editing",async()=>{
 desktop();const world={...populated,stations:[...populated.stations,{id:"4",owner:"2",name:"東",latitude:35.68,longitude:139.8}],routes:[{id:"6",owner:"2",name:"新方式本線",start:"3"}],alignments:[{id:"5",owner:"2",name:"東西線形",points:[{latitude:35.675,longitude:139.75},{latitude:35.68,longitude:139.8}]}],route_paths:[{route_id:"6",segments:[{kind:"alignment",alignment_id:"5",from:"3",to:"4"}],member_stations:["3","4"]}]};
 vi.mocked(invoke).mockImplementation(async command=>command==="engine_info"?{name:"Rosenfu",version:"test-version"}:world);
 renderStationEditor();await screen.findByText(/test-version/);
 fireEvent.click(screen.getByRole("tab",{name:"路線"}));fireEvent.click(screen.getByRole("button",{name:"編集: 新方式本線"}));
 expect(screen.getByText("所属駅: 中央 → 東")).toBeTruthy();expect(screen.getByText(/建設線形 東西線形/)).toBeTruthy();
 expect(screen.queryByLabelText("路線名")).toBeNull();
 expect(vi.mocked(invoke).mock.calls.some(([command])=>command==="save_route")).toBe(false);
 fireEvent.click(screen.getByRole("tab",{name:"建設"}));fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));fireEvent.click(screen.getByRole("tab",{name:"駅"}));fireEvent.click(screen.getByRole("button",{name:/^中央 35/}));
 expect(screen.getAllByText(/路線: 新方式本線/).length).toBeGreaterThan(0);
});
test("candidate apply invokes RoutePath save and marks world unsaved",async()=>{
 desktop();const stations=[...populated.stations,{id:"4",owner:"2",name:"東",latitude:35.68,longitude:139.8}];
 const path={stations:["3","4"],segments:[{kind:"alignment",alignment_id:"5",from:"3",to:"4"}]};
 const oldRoute={id:"7",owner:"2",name:"既存路線",start:"3"};const oldPath={route_id:"7",segments:path.segments,member_stations:path.stations};const world={...empty,stations,routes:[oldRoute],route_paths:[oldPath]};const applied={...world,routes:[oldRoute,{id:"6",owner:"2",name:"候補路線",start:"3"}],route_paths:[oldPath,{route_id:"6",segments:path.segments,member_stations:path.stations}]};let rejectSave=true;
 vi.mocked(invoke).mockImplementation(async command=>{if(command==="engine_info")return {name:"Rosenfu",version:"test-version"};if(command==="world_snapshot")return world;if(command==="preview_route_candidates")return {status:"unique",candidates:[path],truncated:false};if(command==="preview_route_membership")return {path,member_stations:path.stations,excluded_stations:[]};if(command==="save_planned_route"){if(rejectSave){rejectSave=false;throw Error("temporary route save failure");}return applied;}throw Error(command);});
 render(<App/>);await screen.findByText(/test-version/);fireEvent.click(screen.getByRole("tab",{name:"路線"}));fireEvent.click(screen.getByRole("button",{name:"編集: 既存路線"}));const routePanel=region("#workspace-panel-routes");
 fireEvent.change(routePanel.getByLabelText("候補の起点"),{target:{value:"3"}});fireEvent.change(routePanel.getByLabelText("候補の終点"),{target:{value:"4"}});fireEvent.click(routePanel.getByRole("button",{name:"経路候補を検索"}));
 await routePanel.findByLabelText("候補から作る路線名");fireEvent.change(routePanel.getByLabelText("候補から作る路線名"),{target:{value:"候補路線"}});fireEvent.click(routePanel.getByRole("button",{name:"候補を新規路線へ適用"}));
 await screen.findByText(/temporary route save failure/);expect((routePanel.getByRole("button",{name:"候補を新規路線へ適用"}) as HTMLButtonElement).disabled).toBe(false);expect(routePanel.getByLabelText("候補から作る路線名")).toBeTruthy();fireEvent.click(routePanel.getByRole("button",{name:"候補を新規路線へ適用"}));
 await screen.findByText(/路線と所属駅を適用しました/);expect(routePanel.getByRole("button",{name:"編集: 既存路線"})).toBeTruthy();expect(invoke).toHaveBeenCalledWith("save_planned_route",{routeId:null,name:"候補路線",start:"3",segments:path.segments,excludedStations:[]});expect(routePanel.getByRole("button",{name:"編集: 候補路線"})).toBeTruthy();await screen.findByText("未保存の変更あり");
});
test("reference station import stays separate from game stations and can be hidden",async()=>{
 vi.mocked(isTauri).mockReturnValue(false);render(<App/>);
 fireEvent.click(screen.getByRole("button",{name:"自作サンプル（オフライン）"}));
 const pack={format:"rosenfu-reference-stations",version:1,dataset:{id:"reference-test",revision:"r1",source:"synthetic",license:"test",bounds:{west:139,east:140,south:35,north:36}},stations:[{osmType:"node",osmId:"123",name:"<script>実在参考</script>",longitude:139.75,latitude:35.67504,kind:"station"}]};
 const before=document.querySelectorAll('.station-marker').length;
 fireEvent.change(screen.getByLabelText("参照駅JSON"),{target:{files:[{size:600,text:()=>Promise.resolve(JSON.stringify(pack))}]}});
 await screen.findByText("参考: <script>実在参考</script>");expect(document.querySelectorAll('.reference-station')).toHaveLength(1);expect(document.querySelector('.reference-station script')).toBeNull();expect(within(screen.getByLabelText("地図の出典")).getByRole("link",{name:"© OpenStreetMap contributors"}).getAttribute("href")).toBe("https://www.openstreetmap.org/copyright");expect(document.querySelectorAll('.station-marker')).toHaveLength(before);expect(invoke).not.toHaveBeenCalled();
 fireEvent.click(screen.getByLabelText("参考駅を表示"));expect(document.querySelectorAll('.reference-station')).toHaveLength(0);
 fireEvent.click(screen.getByLabelText("参考駅を表示"));expect(document.querySelectorAll('.reference-station')).toHaveLength(1);
});
test("display settings restore to actual UI and failures are visible outside collapsed settings",()=>{
 vi.mocked(isTauri).mockReturnValue(false);localStorage.setItem("rosenfu.display-settings",JSON.stringify({version:1,settings:{theme:"dark",lineWidth:6,contrast:120,panelWidth:360,textSize:14}}));const view=render(<App/>);const main=document.querySelector("main")!;expect(main.getAttribute("data-theme")).toBe("dark");expect(main.style.getPropertyValue("--panel-width")).toBe("360px");fireEvent.click(screen.getByRole("button",{name:"表示設定を既定に戻す"}));expect(main.getAttribute("data-theme")).toBe("light");expect(localStorage.getItem("rosenfu.display-settings")).toBeNull();view.unmount();
 localStorage.setItem("rosenfu.display-settings","invalid");render(<App/>);expect(document.querySelector('.display-settings-warning')?.textContent).toContain("表示設定の復元に失敗");expect(document.querySelector('.display-settings-warning')?.closest('details')).toBeNull();
});


test("physical path UI removes obsolete manual creation and retains read-only alignment preview",()=>{
 vi.mocked(isTauri).mockReturnValue(false);const view=render(<App/>);
 expect(view.container.querySelector(".alignment-hit")).toBeTruthy();
 expect(view.container.querySelector(".track-connection")).toBeNull();
 fireEvent.click(screen.getByRole("tab",{name:"路線"}));
 expect(screen.queryByLabelText("接続元の駅")).toBeNull();
 expect(screen.queryByLabelText("次にたどる線路")).toBeNull();
 expect(screen.queryByRole("button",{name:"線路を接続"})).toBeNull();
 fireEvent.click(screen.getByRole("tab",{name:"系統"}));
 expect(screen.queryByLabelText("系統で次にたどる線路")).toBeNull();
 expect(screen.queryByRole("button",{name:"系統を適用"})).toBeNull();
 expect(invoke).not.toHaveBeenCalled();
});
