// @vitest-environment jsdom
import React,{useState} from "react";
import {afterEach,expect,test,vi} from "vitest";
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {ReferenceStationControls} from "./ReferenceStationControls";
import type {ReferenceStationStorage} from "./referenceStationStorage";
import type {ReferencePack} from "./referenceStations";
const fixture={format:"rosenfu-reference-stations",version:1,dataset:{id:"local-test",revision:"r1",source:"synthetic",license:"test",bounds:{west:139,east:140,south:35,north:36}},stations:[]};
afterEach(cleanup);
function Harness({storage}:{storage?:ReferenceStationStorage}={}){const [pack,onPack]=useState<ReferencePack|null>(null);const [visible,onVisible]=useState(true);return <ReferenceStationControls storage={storage} {...{pack,onPack,visible,onVisible}} total={0} shown={0}/>;}
const load=(text:string,size=text.length)=>fireEvent.change(screen.getByLabelText("参照駅JSON"),{target:{files:[{size,text:()=>Promise.resolve(text)}]}});
test("local import preserves current pack on invalid or oversized replacements",async()=>{
 render(<Harness/>);load(JSON.stringify(fixture));await screen.findByText("local-test / r1 / 0件");
 load('{"version":99}');await screen.findByRole("alert");expect(screen.getByText("local-test / r1 / 0件")).toBeTruthy();
 load("{}",6*1024*1024);await screen.findByText(/5MiB以下/);expect(screen.getByText("local-test / r1 / 0件")).toBeTruthy();
 fireEvent.click(screen.getByLabelText("参考駅を表示"));expect((screen.getByLabelText("参考駅を表示") as HTMLInputElement).checked).toBe(false);
 fireEvent.click(screen.getByRole("button",{name:"参照パックを外す"}));expect(screen.queryByText("local-test / r1 / 0件")).toBeNull();
});
test("late file completion cannot replace a later import",async()=>{
 render(<Harness/>);let finish!:(text:string)=>void;
 fireEvent.change(screen.getByLabelText("参照駅JSON"),{target:{files:[{size:500,text:()=>new Promise<string>(resolve=>{finish=resolve;})}]}});
 load(JSON.stringify({...fixture,dataset:{...fixture.dataset,id:"newer"}}));await screen.findByText("newer / r1 / 0件");
 finish(JSON.stringify(fixture));await Promise.resolve();expect(screen.queryByText("local-test / r1 / 0件")).toBeNull();
});
test("removing a pack invalidates an in-flight replacement",async()=>{
 render(<Harness/>);load(JSON.stringify(fixture));await screen.findByText("local-test / r1 / 0件");
 let finish!:(text:string)=>void;fireEvent.change(screen.getByLabelText("参照駅JSON"),{target:{files:[{size:500,text:()=>new Promise<string>(resolve=>{finish=resolve;})}]}});
 fireEvent.click(screen.getByRole("button",{name:"参照パックを外す"}));finish(JSON.stringify(fixture));await Promise.resolve();await Promise.resolve();expect(screen.queryByText("local-test / r1 / 0件")).toBeNull();
});
test("unmount ignores file completion and metadata is rendered as text",async()=>{
 const onPack=vi.fn();const {unmount}=render(<ReferenceStationControls pack={null} visible={true} onPack={onPack} onVisible={()=>{}} total={0} shown={0}/>);let finish!:(text:string)=>void;
 fireEvent.change(screen.getByLabelText("参照駅JSON"),{target:{files:[{size:500,text:()=>new Promise<string>(resolve=>{finish=resolve;})}]}});unmount();finish(JSON.stringify(fixture));await Promise.resolve();await Promise.resolve();expect(onPack).not.toHaveBeenCalled();expect(screen.queryByText("local-test / r1 / 0件")).toBeNull();
 render(<Harness/>);const source='<img src="x" onerror="alert(1)">';load(JSON.stringify({...fixture,dataset:{...fixture.dataset,source}}));await screen.findByText(/<img src=/);expect(document.querySelector('.reference-controls img')).toBeNull();expect(screen.getByRole('link',{name:'© OpenStreetMap contributors'}).getAttribute('href')).toBe('https://www.openstreetmap.org/copyright');
});

const memoryStorage=(loaded:string|null=null):ReferenceStationStorage=>({load:vi.fn().mockResolvedValue(loaded),save:vi.fn().mockResolvedValue(undefined),remove:vi.fn().mockResolvedValue(undefined)});
test("startup validates persisted pack and corrupt restoration never mutates current pack",async()=>{
 const storage=memoryStorage(JSON.stringify(fixture));const {unmount}=render(<Harness storage={storage}/>);await screen.findByText("local-test / r1 / 0件");await screen.findByText("端末保存から復元しました。");unmount();
 render(<Harness storage={memoryStorage('{"version":99}')}/>);await screen.findByText(/端末保存の復元に失敗/);expect(screen.queryByText("local-test / r1 / 0件")).toBeNull();expect(screen.getByRole("button",{name:"参照パックを外す"})).toBeTruthy();
});
test("quota and deletion failures stay explicit while session data remains usable",async()=>{
 const storage=memoryStorage();vi.mocked(storage.save).mockRejectedValue(new DOMException("full","QuotaExceededError"));vi.mocked(storage.remove).mockRejectedValue(Error("blocked"));
 render(<Harness storage={storage}/>);load(JSON.stringify(fixture));await screen.findByText(/端末保存に失敗。このセッションでは利用できます/);expect(screen.getByText("local-test / r1 / 0件")).toBeTruthy();
 fireEvent.click(screen.getByRole("button",{name:"参照パックを外す"}));await screen.findByText(/端末データの削除に失敗/);expect(screen.queryByText("local-test / r1 / 0件")).toBeNull();expect(storage.remove).toHaveBeenCalledOnce();
});
test("a late startup restore cannot replace an explicitly imported pack",async()=>{
 let finish!:(json:string)=>void;const storage=memoryStorage();vi.mocked(storage.load).mockReturnValue(new Promise(resolve=>{finish=resolve;}));
 render(<Harness storage={storage}/>);load(JSON.stringify({...fixture,dataset:{...fixture.dataset,id:"new-choice"}}));await screen.findByText("new-choice / r1 / 0件");finish(JSON.stringify(fixture));await Promise.resolve();await Promise.resolve();expect(screen.queryByText("local-test / r1 / 0件")).toBeNull();expect(storage.save).toHaveBeenCalledOnce();
});

test("invalid replacement does not leave an earlier save status pending",async()=>{
 const storage=memoryStorage();let finish!:()=>void;vi.mocked(storage.save).mockReturnValue(new Promise(resolve=>{finish=resolve;}));
 render(<Harness storage={storage}/>);load(JSON.stringify(fixture));await screen.findByText("端末に保存中…");load('{"version":99}');await screen.findByRole("alert");finish();await screen.findByText("端末に保存済み。次回起動で復元します。");expect(screen.getByText("local-test / r1 / 0件")).toBeTruthy();
});
test("invalid file during startup cannot leave restoration pending",async()=>{
 const storage=memoryStorage();let finish!:(json:string)=>void;vi.mocked(storage.load).mockReturnValue(new Promise(resolve=>{finish=resolve;}));
 render(<Harness storage={storage}/>);load('{"version":99}');await screen.findByRole("alert");finish(JSON.stringify(fixture));await screen.findByText("端末保存の確認を完了しました。現在の選択を優先しています。");expect(screen.queryByText("端末に保存した参照パックを確認中…")).toBeNull();
});
test("invalid replacement still reports a prior pending save failure",async()=>{
 const storage=memoryStorage();let fail!:(reason:Error)=>void;vi.mocked(storage.save).mockReturnValue(new Promise((_resolve,reject)=>{fail=reject;}));
 render(<Harness storage={storage}/>);load(JSON.stringify(fixture));await screen.findByText("端末に保存中…");load('{"version":99}');await screen.findByRole("alert");fail(Error("quota exhausted"));await screen.findByText(/端末保存に失敗。このセッションでは利用できます/);expect(screen.getByText("local-test / r1 / 0件")).toBeTruthy();
});
