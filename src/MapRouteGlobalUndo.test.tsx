// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {App} from './App';
vi.mock('@tauri-apps/api/core',()=>({isTauri:()=>true,invoke:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});

test('saved map endpoint edit round-trips through global Undo/Redo with original membership exclusion',async()=>{
 const stations=[{id:'3',owner:'2',name:'東',latitude:35.675,longitude:139.75},{id:'5',owner:'2',name:'中間',latitude:35.675,longitude:139.755},{id:'4',owner:'2',name:'西',latitude:35.675,longitude:139.76}];
 const segment={kind:'alignment',alignment_id:'a',from:'3',to:'4'};
 const before={next_id:'10',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations,services:[],routes:[{id:'r',owner:'2',name:'既存路線',start:'3'}],alignments:[{id:'a',owner:'2',name:'線',points:[stations[0],stations[2]]}],station_placements:[{station_id:'3',alignment_id:'a',position:0},{station_id:'5',alignment_id:'a',position:.5},{station_id:'4',alignment_id:'a',position:1}],route_paths:[{route_id:'r',segments:[segment],member_stations:['3','4'],excluded_stations:['5']}]};
 const shortened={stations:['5','4'],segments:[{...segment,from:'5'}]};
 const after={...before,routes:[{...before.routes[0],name:'短縮路線',start:'5'}],route_paths:[{route_id:'r',segments:shortened.segments,member_stations:['5','4'],excluded_stations:[]}]};
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'map-global-history-test'};
  if(command==='list_save_slots')return {active_slot_id:null,slots:[]};
  if(command==='preview_route_candidates')return {status:'unique',candidates:[shortened],truncated:false};
  if(command==='preview_route_membership')return {path:shortened,member_stations:shortened.stations,excluded_stations:[]};
  if(command==='save_planned_route'||command==='redo_world_edit')return after;
  if(command==='undo_world_edit'||command==='world_snapshot')return before;
  throw Error('unexpected '+command);
 });
 const view=render(<App/>);await screen.findByText(/map-global-history-test/);await act(async()=>{});
 fireEvent.click(screen.getByRole('tab',{name:'路線'}));
 fireEvent.click(screen.getByRole('button',{name:'編集: 既存路線'}));
 fireEvent.click(screen.getByRole('button',{name:'地図で路線を編集'}));
 expect(screen.getByRole('list',{name:'所属駅'}).textContent).toBe('東西');
 await act(async()=>fireEvent.click(view.container.querySelectorAll('.station-marker')[0]));
 await waitFor(()=>expect(screen.getByText('所属駅: 中間 → 西')).toBeTruthy());
 fireEvent.change(screen.getByLabelText('路線名'),{target:{value:'短縮路線'}});
 await act(async()=>fireEvent.click(screen.getByRole('button',{name:'編集終了'})));
 await screen.findByRole('button',{name:'編集: 短縮路線'});
 expect(invoke).toHaveBeenCalledWith('save_planned_route',{routeId:'r',name:'短縮路線',start:'5',segments:shortened.segments,excludedStations:[]});
 for(let cycle=0;cycle<2;cycle++){
  await act(async()=>fireEvent.keyDown(document.body,{key:'z',ctrlKey:true}));
  await screen.findByRole('button',{name:'編集: 既存路線'});
  expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==='undo_world_edit').at(-1)?.[1]).toEqual(expect.objectContaining({expectedWorld:expect.objectContaining({route_paths:after.route_paths})}));
  fireEvent.click(screen.getByRole('button',{name:'編集: 既存路線'}));
  fireEvent.click(screen.getByRole('button',{name:'地図で路線を編集'}));
  expect(screen.getByRole('list',{name:'所属駅'}).textContent).toBe('東西');
  fireEvent.click(screen.getByRole('button',{name:'キャンセル'}));
  await act(async()=>fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true}));
  await screen.findByRole('button',{name:'編集: 短縮路線'});
  expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==='redo_world_edit').at(-1)?.[1]).toEqual(expect.objectContaining({expectedWorld:expect.objectContaining({route_paths:before.route_paths})}));
 }
 expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==='save_planned_route')).toHaveLength(1);
 expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==='undo_world_edit')).toHaveLength(2);
 expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==='redo_world_edit')).toHaveLength(2);
});
