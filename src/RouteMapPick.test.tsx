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
 fireEvent.click(screen.getByRole('tab',{name:'路線'}));
 return {stations:view.container.querySelectorAll('.station-marker'),map:screen.getByRole('img',{name:'駅配置用OpenStreetMap'})};
}
const begin=()=>fireEvent.click(screen.getByRole('button',{name:'新規路線'}));
test('new route immediately opens a named sequential map editor without legacy picker controls',async()=>{
 const {stations,map}=await setup();
 expect(screen.queryByRole('region',{name:'路線を作成'})).toBeNull();begin();
 expect((screen.getByLabelText('路線名') as HTMLInputElement).value).toBe('路線1');
 expect(map.classList.contains('route-station-select')).toBe(true);
 for(const label of ['地図で起点を選択','地図で終点を選択','駅を順に選択','経路候補を検索'])expect(screen.queryByRole('button',{name:label})).toBeNull();
 expect(screen.queryByLabelText('候補の起点')).toBeNull();
 fireEvent.click(stations[0]);fireEvent.click(stations[1]);
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('preview_route_candidates',{start:'3',end:'4'}));
 expect(screen.getByRole('tab',{name:'路線'}).getAttribute('aria-selected')).toBe('true');
 expect((screen.getByRole('button',{name:'編集終了'}) as HTMLButtonElement).disabled).toBe(true);
 expect(vi.mocked(invoke).mock.calls.some(([name])=>/^(save_planned_route|rename_station|add_station|delete_station)$/.test(name))).toBe(false);
});
test('Escape and leaving route tab stop station capture without persisting a draft',async()=>{
 const {stations,map}=await setup();begin();fireEvent.keyDown(window,{key:'Escape'});
 expect(map.classList.contains('route-station-select')).toBe(false);
 fireEvent.click(stations[0]);expect(screen.getByRole('tab',{name:'駅'}).getAttribute('aria-selected')).toBe('true');
 fireEvent.click(screen.getByRole('tab',{name:'路線'}));
 fireEvent.click(within(screen.getByRole('region',{name:'路線を作成'})).getByRole('button',{name:'キャンセル'}));
 begin();fireEvent.click(screen.getByRole('tab',{name:'財務'}));
 expect(map.classList.contains('route-station-select')).toBe(false);
 fireEvent.click(stations[1]);expect(screen.getByRole('tab',{name:'駅'}).getAttribute('aria-selected')).toBe('true');
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='save_planned_route')).toBe(false);
});
test('ordered station clicks automatically search with intermediate stations in order and reject repeated stations',async()=>{
 const {stations}=await setup();begin();
 for(const index of [0,2,3,1]){fireEvent.click(stations[index]);await act(async()=>{});}
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('preview_route_candidates',{start:'3',end:'4',viaStationIds:['5','6']}));
 const before=vi.mocked(invoke).mock.calls.filter(([c])=>c==='preview_route_candidates').length;
 fireEvent.click(stations[2]);await act(async()=>{});
 expect(screen.getByRole('alert').textContent).toContain('重複');
 expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==='preview_route_candidates')).toHaveLength(before);
 fireEvent.click(within(screen.getByRole('region',{name:'路線を作成'})).getByRole('button',{name:'キャンセル'}));
 expect(screen.queryByRole('region',{name:'路線を作成'})).toBeNull();
});
