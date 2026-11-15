// @vitest-environment jsdom
import React from "react";
import {afterEach,expect,test,vi} from "vitest";
import {cleanup,fireEvent,render,screen,within} from "@testing-library/react";
import {invoke,isTauri} from "@tauri-apps/api/core";
import {App} from "./App";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn(),isTauri:vi.fn()}));afterEach(()=>{cleanup();vi.resetAllMocks();});
const world={next_id:"9",players:[{id:"100",name:"Player"}],companies:[{id:"200",owner:"100",name:"Railway"}],stations:[{id:"1",owner:"200",name:"中央",latitude:35.67,longitude:139.75},{id:"2",owner:"200",name:"途中",latitude:35.68,longitude:139.75},{id:"3",owner:"200",name:"終点",latitude:35.69,longitude:139.75}],alignments:[{id:"4",owner:"200",name:"線形",track_type:"normal",points:[{latitude:35.67,longitude:139.75},{latitude:35.69,longitude:139.75}]}],station_placements:["1","2","3"].map((station_id,i)=>({station_id,alignment_id:"4",position:i/2,length_m:100,width_m:12})),routes:[{id:"6",owner:"200",name:"参考路線",start:"1"}],route_paths:[{route_id:"6",segments:[{kind:"alignment",alignment_id:"4",from:"1",to:"3"}],member_stations:["1","3"]}],services:[{id:"7",owner:"200",name:"既存系統",start:"1"}],service_paths:[{service_id:"7",next_leg_key:"2",legs:[{key:"1",segment:{kind:"alignment",alignment_id:"4",from:"1",to:"3"}}],stops:[{kind:"start"},{kind:"on_leg",leg_key:"1",station_id:"3"}]}]};

test('binding edits are explicit, protect unsaved selection and preserve route-independent state on rejection',async()=>{
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'binding-integration'};
  if(command==='set_service_route_binding')throw Error('経路が路線に含まれません');
  return world;
 });
 render(<App/>);await screen.findByText(/binding-integration/);fireEvent.click(screen.getByRole('tab',{name:'系統'}));fireEvent.click(screen.getByRole('button',{name:'系統編集: 既存系統'}));
 expect((screen.getByLabelText('関連する路線') as HTMLSelectElement).value).toBe('');
 fireEvent.change(screen.getByLabelText('関連する路線'),{target:{value:'route:6'}});
 fireEvent.click(screen.getByRole('button',{name:'新規系統'}));expect(screen.getByLabelText('関連する路線')).toBeTruthy();
 fireEvent.click(screen.getByRole('button',{name:'関連付けを適用'}));await screen.findByText('Error: 経路が路線に含まれません');
 expect(invoke).toHaveBeenCalledWith('set_service_route_binding',{serviceId:'7',binding:{kind:'route',route_id:'6'}});
 expect((screen.getByLabelText('関連する路線') as HTMLSelectElement).value).toBe('route:6');
 fireEvent.click(screen.getByRole('button',{name:'関連付けの変更を破棄'}));expect((screen.getByLabelText('関連する路線') as HTMLSelectElement).value).toBe('');
 fireEvent.click(screen.getByRole('button',{name:'新規系統'}));expect(screen.queryByLabelText('関連する路線')).toBeNull();
});
