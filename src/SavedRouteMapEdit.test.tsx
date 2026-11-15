// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {App} from './App';
vi.mock('@tauri-apps/api/core',()=>({isTauri:()=>true,invoke:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
const stations=[{id:'3',owner:'2',name:'東',latitude:35.675,longitude:139.75},{id:'4',owner:'2',name:'西',latitude:35.675,longitude:139.76}];
const route={id:'r',owner:'2',name:'既存路線',start:'3',tracks:['t']};
const world={next_id:'10',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations,tracks:[{id:'t',owner:'2',from:'3',to:'4'}],routes:[route],services:[]};
const path={stations:['4','3'],segments:[{kind:'alignment',alignment_id:'a',from:'4',to:'3'}]};
async function setup(fail=false){
 vi.mocked(invoke).mockImplementation(async(command)=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'saved-route-map-test'};
  if(command==='preview_route_candidates')return {status:'unique',candidates:[path],truncated:false};
  if(command==='preview_route_membership')return {path,member_stations:path.stations,excluded_stations:[]};
  if(command==='save_planned_route'){if(fail)throw Error('系統が使用中のため経路を変更できません。');return {...world,routes:[{...route,name:'変更路線',start:'4',tracks:[]}],route_paths:[{route_id:'r',segments:path.segments,member_stations:path.stations}]};}
  return world;
 });
 const view=render(<App/>);await screen.findByText(/saved-route-map-test/);
 fireEvent.click(screen.getByRole('tab',{name:'路線'}));fireEvent.click(screen.getByRole('button',{name:'編集: 既存路線'}));fireEvent.click(screen.getByRole('button',{name:'地図で路線の経路を選び直す'}));
 return view.container.querySelectorAll('.station-marker');
}
test('editing saved route sends original ID and preserves name until explicit apply',async()=>{
 const markers=await setup();fireEvent.click(markers[1]);fireEvent.click(markers[0]);
 const name=await screen.findByLabelText('変更後の路線名');expect((name as HTMLInputElement).value).toBe('既存路線');
 fireEvent.change(name,{target:{value:'変更路線'}});fireEvent.click(screen.getByRole('button',{name:'路線の経路を更新'}));
 await screen.findByRole('button',{name:'編集: 変更路線'});
 expect(invoke).toHaveBeenCalledWith('save_planned_route',{routeId:'r',name:'変更路線',start:'4',segments:path.segments,excludedStations:[]});
 expect(vi.mocked(invoke).mock.calls.some(([command])=>command.includes('service'))).toBe(false);
});
test('backend rejection keeps candidate and original route; cancelling never writes',async()=>{
 const markers=await setup(true);fireEvent.click(markers[1]);fireEvent.click(markers[0]);await screen.findByLabelText('変更後の路線名');
 fireEvent.click(screen.getByRole('button',{name:'路線の経路を更新'}));await screen.findByText(/候補の確認に失敗しました.*系統が使用中/);
 expect((screen.getByLabelText('変更後の路線名') as HTMLInputElement).value).toBe('既存路線');
 expect(screen.getByRole('button',{name:'編集: 既存路線'})).toBeTruthy();
 fireEvent.click(screen.getByRole('button',{name:'新規路線'}));
 expect(screen.getByLabelText('変更後の路線名')).toBeTruthy();
 const count=vi.mocked(invoke).mock.calls.filter(([c])=>c==='save_planned_route').length;
 fireEvent.click(screen.getByRole('button',{name:'経路変更を取り消す'}));
 expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==='save_planned_route')).toHaveLength(count);
 expect(screen.queryByRole('button',{name:'路線の経路を更新'})).toBeNull();
});
