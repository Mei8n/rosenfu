// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {App} from './App';
vi.mock('@tauri-apps/api/core',()=>({isTauri:()=>true,invoke:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
const world={next_id:'10',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[{id:'3',owner:'2',name:'起点駅',latitude:35.675,longitude:139.75},{id:'4',owner:'2',name:'終点駅',latitude:35.675,longitude:139.76}],tracks:[],routes:[],services:[]};
world.stations.push({id:'5',owner:'2',name:'経由一',latitude:35.675,longitude:139.755},{id:'6',owner:'2',name:'経由二',latitude:35.675,longitude:139.757});
async function setup(){
 vi.mocked(invoke).mockImplementation(async command=>command==='engine_info'?{name:'Rosenfu',version:'route-pick-test'}:command==='preview_route_candidates'?{status:'no_path',candidates:[],truncated:false}:world);
 const view=render(<App/>);await screen.findByText(/route-pick-test/);
 fireEvent.click(screen.getByRole('tab',{name:'路線'}));
 const stations=view.container.querySelectorAll('.station-marker');
 return {stations};
}
test('map picks route endpoints without opening station management or changing world',async()=>{
 const {stations}=await setup();
 fireEvent.click(screen.getByRole('button',{name:'地図で起点を選択'}));fireEvent.click(stations[0]);
 expect((screen.getByLabelText('候補の起点') as HTMLSelectElement).value).toBe('3');
 expect(screen.getByRole('tab',{name:'路線'}).getAttribute('aria-selected')).toBe('true');
 fireEvent.click(screen.getByRole('button',{name:'地図で終点を選択'}));fireEvent.click(stations[1]);
 expect((screen.getByLabelText('候補の終点') as HTMLSelectElement).value).toBe('4');
 fireEvent.click(screen.getByRole('button',{name:'経路候補を検索'}));
 await screen.findByText(/現在の接続関係では経路が見つかりません/);
 expect(invoke).toHaveBeenCalledWith('preview_route_candidates',{start:'3',end:'4'});
 expect(vi.mocked(invoke).mock.calls.some(([name])=>/^(save_planned_route|rename_station|add_station|delete_station)$/.test(name))).toBe(false);
});
test('Escape, toggling picker and switching tabs cancel map selection',async()=>{
 const {stations}=await setup();
 const pick=()=>fireEvent.click(screen.getByRole('button',{name:'地図で起点を選択'}));
 pick();fireEvent.keyDown(window,{key:'Escape'});fireEvent.click(stations[0]);
 expect(screen.getByRole('tab',{name:'駅'}).getAttribute('aria-selected')).toBe('true');
 fireEvent.click(screen.getByRole('tab',{name:'路線'}));pick();pick();fireEvent.click(stations[0]);
 expect(screen.getByRole('tab',{name:'駅'}).getAttribute('aria-selected')).toBe('true');
 fireEvent.click(screen.getByRole('tab',{name:'路線'}));pick();fireEvent.click(screen.getByRole('tab',{name:'財務'}));fireEvent.click(stations[1]);
 expect(screen.getByRole('tab',{name:'駅'}).getAttribute('aria-selected')).toBe('true');
});

test('ordered map waypoints are editable, reject duplicates and are sent to constrained search',async()=>{
 const {stations}=await setup();
 fireEvent.click(screen.getByRole('button',{name:'地図で起点を選択'}));fireEvent.click(stations[0]);
 fireEvent.click(screen.getByRole('button',{name:'地図で終点を選択'}));fireEvent.click(stations[1]);
 for(const index of [2,3]){fireEvent.click(screen.getByRole('button',{name:'地図で経由駅を追加'}));fireEvent.click(stations[index]);}
 fireEvent.click(screen.getByRole('button',{name:'経由駅 経由二 を上へ'}));
 fireEvent.click(screen.getByRole('button',{name:'経路候補を検索'}));
 await screen.findByText(/現在の接続関係では経路が見つかりません/);
 expect(invoke).toHaveBeenCalledWith('preview_route_candidates',{start:'3',end:'4',viaStationIds:['6','5']});
 fireEvent.click(screen.getByRole('button',{name:'地図で経由駅を追加'}));fireEvent.click(stations[2]);
 expect(screen.getByRole('alert').textContent).toContain('重複');
 fireEvent.click(screen.getByRole('button',{name:'経由駅 経由二 を除外'}));
 expect(screen.getByRole('list',{name:'経由駅の順序'}).children).toHaveLength(1);
 fireEvent.click(screen.getByRole('button',{name:'候補を閉じる'}));
 expect(screen.getByRole('list',{name:'経由駅の順序'}).children).toHaveLength(0);
});
test('new route accepts sequential map station clicks with the last as endpoint',async()=>{
 const {stations}=await setup();fireEvent.click(screen.getByRole('button',{name:'新規路線'}));
 for(const index of [0,2,3,1])fireEvent.click(stations[index]);
 expect((screen.getByLabelText('候補の起点') as HTMLSelectElement).value).toBe('3');
 expect((screen.getByLabelText('候補の終点') as HTMLSelectElement).value).toBe('4');
 expect(screen.getByRole('list',{name:'経由駅の順序'}).textContent).toContain('経由一');
 fireEvent.click(screen.getByRole('button',{name:'経路候補を検索'}));await screen.findByText(/現在の接続関係では経路が見つかりません/);
 expect(invoke).toHaveBeenCalledWith('preview_route_candidates',{start:'3',end:'4',viaStationIds:['5','6']});
});
