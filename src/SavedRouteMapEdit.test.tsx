// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen,act,waitFor} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {App} from './App';
vi.mock('@tauri-apps/api/core',()=>({isTauri:()=>true,invoke:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
const stations=[{id:'3',owner:'2',name:'東',latitude:35.675,longitude:139.75},{id:'4',owner:'2',name:'西',latitude:35.675,longitude:139.76}];
const route={id:'r',owner:'2',name:'既存路線',start:'3'};
const world={next_id:'10',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations,routes:[route],services:[]};
const path={stations:['4','3'],segments:[{kind:'alignment',alignment_id:'a',from:'4',to:'3'}]};
async function setup(fail=false,membershipGate?:Promise<void>){
 vi.mocked(invoke).mockImplementation(async(command)=>{
  if(command==='list_save_slots')return {active_slot_id:'test-slot',slots:[]};
  if(command==='autosave_status')return {available:false};
  if(command==='engine_info')return {name:'Rosenfu',version:'saved-route-map-test'};
  if(command==='preview_route_candidates')return {status:'unique',candidates:[path],truncated:false};
  if(command==='preview_route_membership'){await membershipGate;return {path,member_stations:path.stations,excluded_stations:[]};}
  if(command==='save_planned_route'){if(fail)throw Error('系統が使用中のため経路を変更できません。');return {...world,routes:[{...route,name:'変更路線',start:'4'}],route_paths:[{route_id:'r',segments:path.segments,member_stations:path.stations}]};}
  return world;
 });
 const view=render(<App/>);await screen.findByText(/saved-route-map-test/);await waitFor(()=>expect(invoke).toHaveBeenCalledWith('autosave_status',{slotId:'test-slot'}));await act(async()=>{});
 fireEvent.click(screen.getByRole('tab',{name:'路線'}));fireEvent.click(screen.getByRole('button',{name:'編集: 既存路線'}));fireEvent.click(screen.getByRole('button',{name:'地図で路線を編集'}));
 return view.container.querySelectorAll('.station-marker');
}
test('editing saved route sends original ID and preserves name until explicit apply',async()=>{
 const markers=await setup();fireEvent.click(markers[1]);fireEvent.click(markers[0]);
 const name=await screen.findByLabelText('路線名');expect((name as HTMLInputElement).value).toBe('既存路線');
 fireEvent.change(name,{target:{value:'変更路線'}});await waitFor(()=>expect((screen.getByRole('button',{name:'編集終了'}) as HTMLButtonElement).disabled).toBe(false));fireEvent.click(screen.getByRole('button',{name:'編集終了'}));
 await screen.findByRole('button',{name:'編集: 変更路線'});
 expect(invoke).toHaveBeenCalledWith('save_planned_route',{routeId:'r',name:'変更路線',start:'4',segments:path.segments,excludedStations:[]});
 expect(vi.mocked(invoke).mock.calls.some(([command])=>command.includes('service'))).toBe(false);
});
test('backend rejection keeps candidate and original route; cancelling never writes',async()=>{
 const markers=await setup(true);fireEvent.click(markers[1]);fireEvent.click(markers[0]);await screen.findByLabelText('路線名');
 await waitFor(()=>expect((screen.getByRole('button',{name:'編集終了'}) as HTMLButtonElement).disabled).toBe(false));fireEvent.click(screen.getByRole('button',{name:'編集終了'}));await screen.findByText(/系統が使用中/);
 expect((screen.getByLabelText('路線名') as HTMLInputElement).value).toBe('既存路線');
 expect(screen.getByRole('button',{name:'編集: 既存路線'})).toBeTruthy();
 fireEvent.click(screen.getByRole('button',{name:'新規路線'}));
 expect(screen.getByLabelText('路線名')).toBeTruthy();
 const count=vi.mocked(invoke).mock.calls.filter(([c])=>c==='save_planned_route').length;
 fireEvent.click(screen.getByRole('button',{name:'キャンセル'}));
 expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==='save_planned_route')).toHaveLength(count);
 expect(screen.queryByRole('button',{name:'編集終了'})).toBeNull();
});

test('route apply waits for membership validation before invoking a backend rejection',async()=>{
 let resolve!:()=>void;const gate=new Promise<void>(r=>{resolve=r;});
 const markers=await setup(true,gate);fireEvent.click(markers[1]);fireEvent.click(markers[0]);await screen.findByLabelText('路線名');
 const finish=screen.getByRole('button',{name:'編集終了'}) as HTMLButtonElement;
 expect(finish.disabled).toBe(true);fireEvent.click(finish);
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='save_planned_route')).toBe(false);
 await act(async()=>{resolve();});await waitFor(()=>expect(finish.disabled).toBe(false));fireEvent.click(finish);
 await screen.findByText(/系統が使用中/);
 expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==='save_planned_route')).toHaveLength(1);
 expect((screen.getByLabelText('路線名') as HTMLInputElement).value).toBe('既存路線');
});
test('one saved segment exposes all physical stations and explicitly choosing a new start shortens to the intermediate station',async()=>{
 const three=[...stations,{...stations[1],id:'5',name:'中間',longitude:139.755}];
 const segment={kind:'alignment',alignment_id:'a',from:'3',to:'4'};
 const physical={...world,stations:three,alignments:[{id:'a',owner:'2',name:'線',points:[stations[0],stations[1]]}],station_placements:[{station_id:'3',alignment_id:'a',position:0},{station_id:'5',alignment_id:'a',position:.5},{station_id:'4',alignment_id:'a',position:1}],route_paths:[{route_id:'r',segments:[segment],member_stations:['3','5','4'],excluded_stations:[]}]};
 const shortened={stations:['5','4'],segments:[{...segment,from:'5'}]};
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'intermediate-test'};
  if(command==='list_save_slots')return {active_slot_id:null,slots:[]};
  if(command==='preview_route_candidates')return {status:'unique',candidates:[shortened],truncated:false};
  if(command==='preview_route_membership')return {path:shortened,member_stations:shortened.stations,excluded_stations:[]};
  return physical;
 });
 const view=render(<App/>);await screen.findByText(/intermediate-test/);await act(async()=>{});
 fireEvent.click(screen.getByRole('tab',{name:'路線'}));fireEvent.click(screen.getByRole('button',{name:'編集: 既存路線'}));fireEvent.click(screen.getByText('地図で路線を編集'));
 expect(screen.getByLabelText('2 経由 中間')).toBeTruthy();
 await act(async()=>{fireEvent.click(view.container.querySelectorAll('.station-marker')[0]);});
 expect(screen.getByText('地図か駅名検索で新しい始点を選択してください。')).toBeTruthy();
 expect(invoke).not.toHaveBeenCalledWith('preview_route_candidates',{start:'5',end:'4'});
 await act(async()=>{fireEvent.click(view.container.querySelectorAll('.station-marker')[1]);}); // opposite endpoint is invalid
 expect(await screen.findByText(/始点と終点に同じ駅を指定できません/)).toBeTruthy();
 expect(screen.getByText('地図か駅名検索で新しい始点を選択してください。')).toBeTruthy();
 const search=screen.getByRole('searchbox',{name:'駅名で検索'});
 fireEvent.keyDown(search,{key:'Escape',isComposing:true,keyCode:229});
 fireEvent.change(search,{target:{value:'中間'}});
 fireEvent.click(screen.getByRole('button',{name:'中間'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('preview_route_candidates',{start:'5',end:'4'}));
 expect(screen.queryByText(/2駅以上必要/)).toBeNull();expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='save_planned_route')).toBe(false);
});

test('saved membership exclusion survives rename and cancelling map edits',async()=>{
 const three=[...stations,{...stations[1],id:'5',name:'中間',longitude:139.755}];
 const segment={kind:'alignment',alignment_id:'a',from:'3',to:'4'};
 const physical={...world,stations:three,alignments:[{id:'a',owner:'2',name:'線',points:[stations[0],stations[1]]}],station_placements:[{station_id:'3',alignment_id:'a',position:0},{station_id:'5',alignment_id:'a',position:.5},{station_id:'4',alignment_id:'a',position:1}],route_paths:[{route_id:'r',segments:[segment],member_stations:['3','4'],excluded_stations:[]}]};
 const shortened={stations:['5','4'],segments:[{...segment,from:'5'}]};
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'intermediate-test'};
  if(command==='list_save_slots')return {active_slot_id:null,slots:[]};
  if(command==='preview_route_candidates')return {status:'unique',candidates:[shortened],truncated:false};
  if(command==='preview_route_membership')return {path:shortened,member_stations:shortened.stations,excluded_stations:[]};
  return physical;
 });
 const view=render(<App/>);await screen.findByText(/intermediate-test/);await act(async()=>{});
 fireEvent.click(screen.getByRole('tab',{name:'路線'}));fireEvent.click(screen.getByRole('button',{name:'編集: 既存路線'}));fireEvent.click(screen.getByText('地図で路線を編集'));
 expect(screen.getByText('所属駅: 東 → 西')).toBeTruthy();
 expect(screen.getByRole('list',{name:'所属駅'}).textContent).toBe('東西');
 fireEvent.change(screen.getByLabelText('路線名'),{target:{value:'名前だけ変更'}});
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='save_planned_route')).toBe(false);
 await act(async()=>fireEvent.click(screen.getByText('編集終了')));
 expect(invoke).toHaveBeenCalledWith('save_planned_route',{routeId:'r',name:'名前だけ変更',start:'3',segments:[segment],excludedStations:['5']});
 fireEvent.click(screen.getByText('地図で路線を編集'));fireEvent.click(screen.getByRole('button',{name:'キャンセル'}));
 expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==='save_planned_route')).toHaveLength(1);
});
