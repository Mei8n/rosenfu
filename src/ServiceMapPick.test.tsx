// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen,waitFor,within} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {App} from './App';
vi.mock('@tauri-apps/api/core',()=>({isTauri:()=>true,invoke:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
const world={next_id:'10',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[{id:'3',owner:'2',name:'起点駅',latitude:35.675,longitude:139.75},{id:'4',owner:'2',name:'終点駅',latitude:35.675,longitude:139.76},{id:'5',owner:'2',name:'経由一',latitude:35.675,longitude:139.755},{id:'6',owner:'2',name:'経由二',latitude:35.675,longitude:139.757}],routes:[],services:[],route_paths:[],service_paths:[]};
async function setup(){
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'route-pick-test'};
  if(command==='list_save_slots')return {active_slot_id:'test-slot',slots:[]};
  if(command==='autosave_status')return {available:false};
  if(command==='preview_route_candidates')return {status:'no_path',candidates:[],truncated:false};
  return world;
 });
 const view=render(<App/>);await screen.findByText(/route-pick-test/);
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('autosave_status',{slotId:'test-slot'}));await act(async()=>{});
 fireEvent.click(screen.getByRole('tab',{name:'系統'}));
 return {stations:view.container.querySelectorAll('.station-marker'),map:screen.getByRole('img',{name:'駅配置用OpenStreetMap'})};
}
const begin=()=>fireEvent.click(screen.getByRole('button',{name:'新規系統'}));
test('new service captures map station clicks directly without editing the station or creating a route',async()=>{
 const {stations,map}=await setup();begin();
 expect((within(screen.getByRole('region',{name:'系統を作成'})).getByLabelText('系統名') as HTMLInputElement).value).toBe('系統1');
 expect(map.classList.contains('route-station-select')).toBe(true);
 fireEvent.click(stations[0]);fireEvent.click(stations[1]);
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('preview_route_candidates',{start:'3',end:'4'}));
 expect(screen.getByRole('tab',{name:'系統'}).getAttribute('aria-selected')).toBe('true');
 expect(screen.getByLabelText('1 始点 起点駅')).toBeTruthy();
 expect(screen.getByLabelText('2 終点 終点駅')).toBeTruthy();
 expect(vi.mocked(invoke).mock.calls.some(([name])=>/^(save_planned_route|rename_station|create_service_path)$/.test(name))).toBe(false);
 fireEvent.keyDown(window,{key:'Escape'});expect(map.classList.contains('route-station-select')).toBe(false);
 fireEvent.click(within(screen.getByRole('region',{name:'系統を作成'})).getByRole('button',{name:'駅の選択を再開'}));expect(map.classList.contains('route-station-select')).toBe(true);
 fireEvent.click(within(screen.getByRole('region',{name:'系統を作成'})).getByRole('button',{name:'キャンセル'}));expect(map.classList.contains('route-station-select')).toBe(false);
 expect(screen.queryByLabelText('系統の選択駅')).toBeNull();
 fireEvent.click(stations[0]);expect(screen.getByRole('tab',{name:'駅'}).getAttribute('aria-selected')).toBe('true');
});

test('service draft labels are not shown after switching to another workspace tab',async()=>{
 const {stations,map}=await setup();begin();fireEvent.click(stations[0]);
 expect(screen.getByLabelText('1 始点 起点駅')).toBeTruthy();
 fireEvent.click(screen.getByRole('tab',{name:'駅'}));
 expect(screen.queryByLabelText('系統の選択駅')).toBeNull();
 expect(map.classList.contains('route-station-select')).toBe(false);
});
