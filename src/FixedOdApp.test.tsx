// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen,within} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {App} from './App';
vi.mock('@tauri-apps/api/core',()=>({isTauri:()=>true,invoke:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
const world={next_id:'10',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[{id:'3',owner:'2',name:'東駅',latitude:35,longitude:139},{id:'4',owner:'2',name:'西駅',latitude:35,longitude:139.01}],routes:[],services:[]};
function mockEngine(available:boolean){
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'od-app-test',fixed_od_enabled:available};
  if(command==='list_save_slots')return {active_slot_id:null,slots:[]};
  if(command==='preview_fixed_od')return {potential_trips:100,rail:[],alternative_trips:0,unserved_trips:100,unresolved_trips:0,search_complete:true};
  return world;
 });
}
test('native OD capability exposes a working city preview and no preserved synthetic demand',async()=>{
 mockEngine(true);render(<App/>);await screen.findByText(/od-app-test/);
 fireEvent.click(screen.getByRole('tab',{name:'都市'}));
 fireEvent.change(within(screen.getByRole("region",{name:"固定ODの需要試算"})).getByLabelText('出発駅'),{target:{value:'3'}});
 fireEvent.change(within(screen.getByRole("region",{name:"固定ODの需要試算"})).getByLabelText('到着駅'),{target:{value:'4'}});
 fireEvent.click(screen.getByRole('button',{name:'需要を試算'}));
 expect(await screen.findByText('100人',{selector:'dd'})).toBeTruthy();
 expect(invoke).toHaveBeenCalledWith('preview_fixed_od',{originStationId:'3',destinationStationId:'4',trips:100,alternativeSeconds:null});
 expect(screen.queryByText('未保存の変更あり')).toBeNull();
});
test('older engine leaves the city panel unavailable',async()=>{
 mockEngine(false);render(<App/>);await screen.findByText(/od-app-test/);
 fireEvent.click(screen.getByRole('tab',{name:'都市'}));
 expect(screen.queryByRole('button',{name:'需要を試算'})).toBeNull();
});

test('explicit recording uses the token captured with initial native read and marks the active world unsaved',async()=>{
 const scenario={origin_station_id:'3',destination_station_id:'4',trips:100,alternative_seconds:null};
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'od-app-test',fixed_od_enabled:true,fixed_od_scenario_enabled:true};
  if(command==='list_save_slots')return {active_slot_id:null,slots:[]};
  if(command==='fixed_od_scenario')return {token:'mounted-world',scenario:null};
  if(command==='set_fixed_od_scenario')return {token:'next-revision',scenario};
  return world;
 });
 render(<App/>);await screen.findByText(/od-app-test/);
 fireEvent.click(screen.getByRole('tab',{name:'都市'}));
 const panel=within(screen.getByRole('region',{name:'固定ODの需要試算'}));
 fireEvent.change(panel.getByLabelText('出発駅'),{target:{value:'3'}});
 fireEvent.change(panel.getByLabelText('到着駅'),{target:{value:'4'}});
 fireEvent.click(panel.getByRole('button',{name:'この試算条件をワールドに記録'}));
 expect(await panel.findByText('記録済みの条件は次のセーブに含まれます。')).toBeTruthy();
 expect(invoke).toHaveBeenCalledWith('set_fixed_od_scenario',{expectedToken:'mounted-world',scenario});
 expect(invoke).not.toHaveBeenCalledWith('ofm_world_token');
 expect(screen.getByText('未保存の変更あり')).toBeTruthy();
});
