// @vitest-environment jsdom
import React from "react";
import {afterEach,expect,test,vi} from "vitest";
import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {invoke} from "@tauri-apps/api/core";
import {App} from "./App";
vi.mock("@tauri-apps/api/core",()=>({isTauri:()=>true,invoke:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
test("map marks only saved route members and shows station dots below detail zoom",async()=>{
 const world={next_id:"10",players:[{id:"1",name:"P"}],companies:[{id:"2",owner:"1",name:"C"}],stations:[{id:"3",owner:"2",name:"所属駅",latitude:35.675,longitude:139.75},{id:"4",owner:"2",name:"未所属駅",latitude:35.675,longitude:139.76}],tracks:[],routes:[{id:"8",owner:"2",name:"路線",start:"3",tracks:[]}],services:[],alignments:[{id:"5",owner:"2",name:"線形",points:[{latitude:35.675,longitude:139.74},{latitude:35.675,longitude:139.77}]}],station_placements:[{station_id:"3",alignment_id:"5",position:.3,length_m:100,width_m:12},{station_id:"4",alignment_id:"5",position:.7,length_m:100,width_m:12}],route_paths:[{route_id:"8",segments:[{kind:"alignment",alignment_id:"5",from:"3",to:"4"}],member_stations:["3"]}]};
 vi.mocked(invoke).mockImplementation(async command=>command==="engine_info"?{name:"Rosenfu",version:"appearance-test"}:world);
 render(<App/>);await screen.findByText(/appearance-test/);
 const map=screen.getByRole("img",{name:"駅配置用OpenStreetMap"});
 expect(map.querySelectorAll('.station-marker circle.station-dot')).toHaveLength(2);
 expect(map.querySelectorAll('.station-marker.route-unassigned')).toHaveLength(1);
 expect(map.querySelector('.station-marker.route-unassigned')?.textContent).toContain('未所属駅');
 expect(map.querySelectorAll('.route-assigned-section')).toHaveLength(1);
 for(let i=0;i<3;i++)fireEvent.click(screen.getByRole('button',{name:'拡大'}));
 await screen.findByText('ZL 15 / Web Mercator');
 await waitFor(()=>expect(map.querySelectorAll('.station-marker circle.station-dot')).toHaveLength(0));
 expect(map.querySelectorAll('.station-marker path.station-platform')).toHaveLength(2);
 fireEvent.click(screen.getByRole('button',{name:'縮小'}));
 await screen.findByText('ZL 14 / Web Mercator');
 expect(map.querySelectorAll('.station-marker circle.station-dot')).toHaveLength(2);
});
