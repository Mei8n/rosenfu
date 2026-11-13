// @vitest-environment jsdom
import React from "react";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { RouteCandidatePreview, type Candidate, type Candidates } from "./RouteCandidatePreview";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
const stations = [{id:"1",name:"始発"},{id:"2",name:"中間"},{id:"3",name:"終点"}];
const path: Candidate = {stations:["1","2","3"],segments:[{kind:"track",track_id:"4"},{kind:"track",track_id:"5"}],track_only:true};
const unique: Candidates = {status:"unique",candidates:[path],truncated:false};
const version = {};
function start(result: Candidates = unique, canApply = true) {
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if(command === "preview_route_candidates") return result;
    if(command === "preview_route_membership") {
      const excluded = (args as {excludedStations:string[]}).excludedStations;
      const chosen = result.candidates.find(item => JSON.stringify(item.segments) === JSON.stringify((args as {segments:unknown}).segments))!;
      return {path:chosen, member_stations:chosen.stations.filter(id => !excluded.includes(id)), excluded_stations:excluded};
    }
    throw Error(command);
  });
  const onApply=vi.fn();
  const props={stations,worldVersion:version,enabled:true,canApply,onApply};
  const view=render(<RouteCandidatePreview {...props}/>);
  fireEvent.change(screen.getByLabelText("候補の起点"),{target:{value:"1"}});
  fireEvent.change(screen.getByLabelText("候補の終点"),{target:{value:"3"}});
  return {...view,onApply,props};
}
const search=()=>fireEvent.click(screen.getByRole("button",{name:"経路候補を検索"}));
test("unique track route previews membership and imports only to a manual draft",async()=>{
  const {onApply}=start(); search();
  await screen.findByText("所属駅: 始発 → 中間 → 終点");
  expect(invoke).toHaveBeenCalledWith("preview_route_candidates",{start:"1",end:"3"});
  expect((screen.getByLabelText("始発（起点）") as HTMLInputElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole("button",{name:"新規路線の手動下書きへ使う"}));
  expect(onApply).toHaveBeenCalledWith("1",["4","5"]);
  expect(screen.queryByText("所属駅: 始発 → 中間 → 終点")).toBeNull();
  expect(vi.mocked(invoke).mock.calls.every(([command])=>command.startsWith("preview_"))).toBe(true);
});
test("ambiguous and incomplete candidates require explicit selection",async()=>{
  const other={...path,segments:[{kind:"track" as const,track_id:"9"}]};
  const {rerender,props}=start({status:"ambiguous",candidates:[path,other],truncated:false}); search();
  await screen.findByText(/複数の経路/);
  expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==="preview_route_membership")).toHaveLength(0);
  expect(screen.getAllByRole("radio").every(r=>!(r as HTMLInputElement).checked)).toBe(true);
  fireEvent.click(screen.getAllByRole("radio")[1]);
  await screen.findByText("所属駅: 始発 → 中間 → 終点");
  rerender(<RouteCandidatePreview {...props} worldVersion={{}}/>);
  vi.mocked(invoke).mockResolvedValueOnce({status:"incomplete",candidates:[path],truncated:true}); search();
  await screen.findByText(/探索上限に達しました/);
  expect((screen.getByRole("radio") as HTMLInputElement).checked).toBe(false);
});
test("exclusion preserves physical segments but blocks import and is reversible",async()=>{
  const {onApply}=start(); search(); await screen.findByText("所属駅: 始発 → 中間 → 終点");
  fireEvent.click(screen.getByLabelText("中間"));
  await screen.findByText("所属駅: 始発 → 終点");
  expect(invoke).toHaveBeenLastCalledWith("preview_route_membership",{start:"1",segments:path.segments,excludedStations:["2"]});
  expect(screen.queryByRole("button",{name:"新規路線の手動下書きへ使う"})).toBeNull();
  expect(onApply).not.toHaveBeenCalled();
  fireEvent.click(screen.getByLabelText("中間"));
  await screen.findByRole("button",{name:"新規路線の手動下書きへ使う"});
});
test("alignment candidates are visible but not saveable",async()=>{
  start({...unique,candidates:[{...path,track_only:false,segments:[{kind:"alignment",alignment_id:"10",from:"1",to:"3"}]}]}); search();
  await screen.findByText("所属駅: 始発 → 中間 → 終点");
  expect(screen.getByText(/この候補はプレビューのみ/)).toBeTruthy();
  expect(screen.queryByRole("button",{name:"新規路線の手動下書きへ使う"})).toBeNull();
});
test("existing drafts prevent replacement and invalid endpoints disable search",async()=>{
  start(unique,false);
  fireEvent.change(screen.getByLabelText("候補の終点"),{target:{value:"1"}});
  expect((screen.getByRole("button",{name:"経路候補を検索"}) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.change(screen.getByLabelText("候補の終点"),{target:{value:"3"}}); search();
  await screen.findByText(/先に編集中の路線下書き/);
  expect((screen.getByRole("button",{name:"新規路線の手動下書きへ使う"}) as HTMLButtonElement).disabled).toBe(true);
});
test("no path, failure and retry are distinguished",async()=>{
  start({status:"no_path",candidates:[],truncated:false}); search();
  await screen.findByText(/現在の接続関係では経路が見つかりません/);
  vi.mocked(invoke).mockRejectedValueOnce(Error("network test")); search();
  await screen.findByRole("alert");
  expect(screen.getByRole("alert").textContent).toContain("network test");
  search(); await screen.findByText(/現在の接続関係では経路が見つかりません/);
  expect(screen.queryByRole("alert")).toBeNull();
});
test("repeated search is deduplicated and endpoint changes discard late results",async()=>{
  start(); let finish!:(value:Candidates)=>void;
  vi.mocked(invoke).mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;}));
  search(); search();
  expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==="preview_route_candidates")).toHaveLength(1);
  fireEvent.change(screen.getByLabelText("候補の終点"),{target:{value:"2"}});
  finish(unique);
  await waitFor(()=>expect(screen.queryByText("経路を確認中…")).toBeNull());
  expect(screen.queryByText("経路候補が見つかりました。")).toBeNull();
  expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==="preview_route_membership")).toHaveLength(0);
});
test("world changes and closing discard pending membership responses",async()=>{
  const {rerender,props}=start(); let finish!:(value:unknown)=>void;
  vi.mocked(invoke).mockResolvedValueOnce(unique).mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;}));
  search(); await screen.findByText("経路候補が見つかりました。");
  rerender(<RouteCandidatePreview {...props} worldVersion={{}}/>);
  finish({path,member_stations:path.stations,excluded_stations:[]});
  await waitFor(()=>expect(screen.queryByText("所属駅: 始発 → 中間 → 終点")).toBeNull());
  search(); await screen.findByText("所属駅: 始発 → 中間 → 終点");
  fireEvent.click(screen.getByRole("button",{name:"候補を閉じる"}));
  expect(screen.queryByText("所属駅: 始発 → 中間 → 終点")).toBeNull();
});
test("persistable candidate saves excluded membership without losing physical segments",async()=>{
 const {rerender,props}=start();const onSave=vi.fn().mockRejectedValueOnce(Error("save rejected")).mockResolvedValueOnce(undefined);
 rerender(<RouteCandidatePreview {...props} onSave={onSave}/>);search();await screen.findByText("所属駅: 始発 → 中間 → 終点");
 fireEvent.click(screen.getByLabelText("中間"));await screen.findByText("所属駅: 始発 → 終点");
 fireEvent.change(screen.getByLabelText("候補から作る路線名"),{target:{value:"環状東線"}});
 fireEvent.click(screen.getByRole("button",{name:"候補を新規路線へ適用"}));await screen.findByText(/save rejected/);
 expect(onSave).toHaveBeenCalledWith("環状東線",{path,member_stations:["1","3"],excluded_stations:["2"]});
 expect(screen.getByText("所属駅: 始発 → 終点")).toBeTruthy();
 fireEvent.click(screen.getByRole("button",{name:"候補を新規路線へ適用"}));await waitFor(()=>expect(screen.queryByText("所属駅: 始発 → 終点")).toBeNull());
});
