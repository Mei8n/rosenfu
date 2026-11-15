import './testConstructionEnvironment';
// @vitest-environment jsdom
import React from "react";
import {afterEach,expect,test,vi} from "vitest";
import {cleanup,fireEvent,render,screen,within} from "@testing-library/react";
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
 for(const platform of document.querySelectorAll("[data-group-platform]")){
  expect(platform.getAttribute("fill")).toBe("white");
  expect(platform.getAttribute("stroke-dasharray")).toBeNull();
 }

 fireEvent.click(screen.getByRole("tab",{name:"駅"}));
 expect(document.querySelectorAll(".station-entry")).toHaveLength(1);
 expect(document.querySelector(".station-directory .count")?.textContent).toBe("1");
 fireEvent.click(screen.getByRole("button",{name:/交差駅/}));
 const homes=screen.getByLabelText("編集するホーム") as HTMLSelectElement;
 expect(homes.options).toHaveLength(2);
 fireEvent.change(homes,{target:{value:"6"}});expect(homes.value).toBe("6");
});

test("old standalone stations remain visible as dots without a platform or group",async()=>{
 vi.mocked(isTauri).mockReturnValue(true);vi.mocked(invoke).mockImplementation(async command=>command==="engine_info"?{version:"legacy-group-test"}:{...world,view_state:{...center,zoom:12},station_placements:[],station_groups:[]});
 render(<App/>);await screen.findByText(/legacy-group-test/);
 const dots=[...document.querySelectorAll(".station-dot")];expect(dots).toHaveLength(2);expect(dots.every(dot=>dot.getAttribute("visibility")!=="hidden")).toBe(true);
});

test("crossing preview reports actual distance without inventing passenger data",async()=>{
 window.PointerEvent=MouseEvent as typeof PointerEvent;
 vi.mocked(isTauri).mockReturnValue(true);vi.mocked(invoke).mockImplementation(async command=>command==="engine_info"?{version:"join-preview-test"}:{...world,stations:world.stations.slice(0,1),station_placements:world.station_placements.slice(0,1),station_groups:[]});
 render(<App/>);await screen.findByText(/join-preview-test/);
 const map=screen.getByRole("img",{name:"駅配置用OpenStreetMap"});
 map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));
 fireEvent.pointerMove(map,{clientX:250,clientY:150});
 const tooltip=screen.getByText("交差駅から 0.00 km").closest(".construction-tooltip");
 expect(tooltip).not.toBeNull();
 expect(within(tooltip as HTMLElement).queryByText(/乗客/)).toBeNull();
 expect(screen.queryByRole("table",{name:"路線一覧"})).toBeNull();
 fireEvent.click(map,{clientX:250,clientY:150});
 expect(document.querySelectorAll(".planned-station")).toHaveLength(1);
 expect(document.querySelectorAll(".station-group-footprint")).toHaveLength(1);

 const platforms=[...document.querySelectorAll("[data-group-platform]")];
 expect(platforms).toHaveLength(2);
 const built=platforms.find(p=>p.getAttribute("data-group-platform")==="5")!;
 const planned=platforms.find(p=>p!==built)!;
 expect(built.getAttribute("fill")).toBe("white");
 expect(built.getAttribute("stroke-dasharray")).toBeNull();
 expect(planned.getAttribute("fill")).toBe("#c4e6f3");
 expect(planned.getAttribute("stroke-dasharray")).toBe("4 3");
 expect(planned.getAttribute("vector-effect")).toBe("non-scaling-stroke");
 expect(planned.textContent).toContain("未着工");
 expect((screen.getByLabelText("計画駅名") as HTMLInputElement).value).toBe("交差駅");
 expect((screen.getByLabelText("計画駅名") as HTMLInputElement).readOnly).toBe(true);
 fireEvent.click(screen.getByRole("button",{name:"別の線路計画を追加"}));
 const inactive=document.querySelector(`[data-group-platform="${planned.getAttribute("data-group-platform")}"]`)!;
 expect(inactive.getAttribute("fill")).toBe("#b1bac3");
 expect(inactive.getAttribute("stroke-dasharray")).toBe("4 3");
 expect(document.querySelector('[data-group-platform="5"]')?.getAttribute("fill")).toBe("white");

});

