// @vitest-environment jsdom
import React from "react";
import {afterEach,expect,test,vi} from "vitest";
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {invoke,isTauri} from "@tauri-apps/api/core";
import {App} from "./App";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
const center={latitude:35.67504,longitude:139.75};
const world={view_state:{...center,zoom:16},next_id:"7",players:[{id:"1",name:"P"}],companies:[{id:"2",owner:"1",name:"C"}],stations:[{id:"5",owner:"2",name:"交差駅",...center},{id:"6",owner:"2",name:"交差駅",...center}],tracks:[],routes:[],services:[],alignments:[{id:"3",owner:"2",name:"横",points:[{...center,longitude:139.73},{...center,longitude:139.77}]},{id:"4",owner:"2",name:"縦",points:[{...center,latitude:35.66},{...center,latitude:35.69}]}],station_placements:[{station_id:"5",alignment_id:"3",position:.5,length_m:200,width_m:12},{station_id:"6",alignment_id:"4",position:.5,length_m:200,width_m:12}],station_groups:[{station_id:"5",members:["5","6"]}]};
test("saved station group has one directory entry and independent home selection",async()=>{
 vi.mocked(isTauri).mockReturnValue(true);vi.mocked(invoke).mockImplementation(async command=>command==="engine_info"?{version:"group-test"}:world);
 render(<App/>);await screen.findByText(/group-test/);
 expect(document.querySelectorAll(".station-group-footprint")).toHaveLength(1);
 fireEvent.click(screen.getByRole("tab",{name:"駅"}));
 expect(document.querySelectorAll(".station-entry")).toHaveLength(1);
 fireEvent.click(screen.getByRole("button",{name:/交差駅/}));
 const homes=screen.getByLabelText("編集するホーム") as HTMLSelectElement;
 expect(homes.options).toHaveLength(2);
 fireEvent.change(homes,{target:{value:"6"}});expect(homes.value).toBe("6");
});
