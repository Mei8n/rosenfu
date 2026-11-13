// @vitest-environment jsdom
import React from "react";
import {afterEach,expect,test,vi} from "vitest";
import {cleanup,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {invoke,isTauri} from "@tauri-apps/api/core";
import {App} from "./App";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
const world={next_id:"7",players:[{id:"1",name:"P"}],companies:[{id:"2",owner:"1",name:"C"}],stations:[{id:"4",owner:"2",name:"第一駅",latitude:35.67,longitude:139.75},{id:"5",owner:"2",name:"第二駅",latitude:35.67,longitude:139.76}],tracks:[],routes:[],services:[],alignments:[{id:"3",owner:"2",name:"A",points:[{latitude:35.67,longitude:139.7},{latitude:35.67,longitude:139.8}]}],station_placements:[{station_id:"4",alignment_id:"3",position:.5,length_m:200,width_m:17},{station_id:"5",alignment_id:"3",position:.6,length_m:200,width_m:12}]};
async function setup(){
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async command=>{if(command==="engine_info")return{name:"R",version:"station-edit"};if(command==="world_snapshot")return world;throw Error(command);});
 render(<App/>);await screen.findByText(/station-edit/);fireEvent.click(screen.getByRole("tab",{name:"駅"}));
 fireEvent.click(within(document.querySelector(".station-directory") as HTMLElement).getByRole("button",{name:/第一駅/}));
 return within(document.querySelector(".equipment-editor") as HTMLElement);
}
test("length draft blocks save and station switching; cancel preserves draft and discard resets",async()=>{
 const editor=await setup();
 expect((editor.getByRole("button",{name:"ホーム寸法を適用"}) as HTMLButtonElement).disabled).toBe(true);
 fireEvent.change(editor.getByLabelText("選択駅のホーム長 (20m級・両相当)"),{target:{value:"12"}});
 expect((screen.getByRole("button",{name:"保存"}) as HTMLButtonElement).disabled).toBe(true);
 fireEvent.click(within(document.querySelector(".station-directory") as HTMLElement).getByRole("button",{name:/第二駅/}));
 expect(screen.getByText("駅名・ホーム長の下書きを破棄して別駅を選びますか。")).toBeTruthy();
 fireEvent.click(screen.getByRole("button",{name:"駅名編集を続ける"}));
 expect((editor.getByLabelText("選択駅のホーム長 (20m級・両相当)") as HTMLInputElement).value).toBe("12");
 fireEvent.click(editor.getByRole("button",{name:"ホーム長の下書きを破棄"}));
 expect((editor.getByLabelText("選択駅のホーム長 (20m級・両相当)") as HTMLInputElement).value).toBe("10");
 fireEvent.change(editor.getByLabelText("選択駅のホーム長 (20m級・両相当)"),{target:{value:"11"}});
 fireEvent.click(within(document.querySelector(".station-directory") as HTMLElement).getByRole("button",{name:/第二駅/}));
 fireEvent.click(screen.getByRole("button",{name:"駅設定下書きを破棄して切替"}));
 expect((editor.getByLabelText("選択駅の名前") as HTMLInputElement).value).toBe("第二駅");
});
test("failed length edit keeps draft; retry preserves stored width and permits saving",async()=>{
 const editor=await setup();const input=editor.getByLabelText("選択駅のホーム長 (20m級・両相当)");
 fireEvent.change(input,{target:{value:"12.5"}});
 vi.mocked(invoke).mockRejectedValueOnce(Error("resize rejected"));
 fireEvent.click(editor.getByRole("button",{name:"ホーム寸法を適用"}));await screen.findByText(/resize rejected/);
 expect((input as HTMLInputElement).value).toBe("12.5");
 vi.mocked(invoke).mockResolvedValueOnce({...world,station_placements:[{...world.station_placements[0],length_m:250},world.station_placements[1]]});
 fireEvent.click(editor.getByRole("button",{name:"ホーム寸法を適用"}));
 await screen.findByText("ホーム寸法を変更しました。保存でファイルに反映します。");
 expect(invoke).toHaveBeenCalledWith("update_station_placement",{stationId:"4",lengthM:250,widthM:17});
 await waitFor(()=>expect((screen.getByRole("button",{name:"保存"}) as HTMLButtonElement).disabled).toBe(false));
 expect((editor.getByRole("button",{name:"ホーム寸法を適用"}) as HTMLButtonElement).disabled).toBe(true);
});
