// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen,within,waitFor} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {App} from './App';
vi.mock('@tauri-apps/api/core',()=>({isTauri:()=>true,invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();localStorage.clear();});
const world={next_id:'20',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[{id:'3',owner:'2',name:'A',latitude:35.675,longitude:139.75},{id:'4',owner:'2',name:'B',latitude:35.675,longitude:139.76}],routes:[{id:'6',owner:'2',name:'本線',start:'3'}],services:[],route_paths:[{route_id:'6',segments:[{kind:'alignment',alignment_id:'5',from:'3',to:'4'}],member_stations:['3','4']}],station_placements:[{station_id:'3',alignment_id:'5',position:.1,length_m:80,width_m:10},{station_id:'4',alignment_id:'5',position:.9,length_m:80,width_m:10}],alignments:[{id:'5',owner:'2',name:'line',points:[{latitude:35.675,longitude:139.75},{latitude:35.675,longitude:139.76}]}]};
async function setup(){
 let fail=true;
 vi.mocked(invoke).mockImplementation(async(command)=>{
  if(command==='engine_info')return{name:'Rosenfu',version:'rename-integration'};
  if(command==='list_save_slots')return{active_slot_id:null,slots:[{id:'slot-second',name:'別ワールド',loadable:true}]};
  if(command==='rename_route'){if(fail){fail=false;throw Error('名前が別の操作で変更されました。');}return{...world,routes:[{...world.routes[0],name:'新本線'}]};}
  if(command==='load_save_slot')return{...world,routes:[{...world.routes[0],name:'別世界線'}]};
  return world;
 });
 render(<App/>);await screen.findByText(/rename-integration/);
 await waitFor(()=>expect(screen.queryByText('保存一覧を読み込み中')).toBeNull());
 fireEvent.click(screen.getByRole('tab',{name:'路線'}));
 fireEvent.click(screen.getByRole('button',{name:'路線名を変更: 本線'}));
 fireEvent.change(screen.getByRole('textbox',{name:'変更後の路線名'}),{target:{value:'新本線'}});
}
test('name edit uses conflict-checked rename only and retains failed input for retry',async()=>{
 await setup();
 const form=screen.getByRole('textbox',{name:'変更後の路線名'}).closest('form')!;
 fireEvent.submit(form);await screen.findByText(/名前が別の操作で変更されました/);
 expect((screen.getByRole('textbox',{name:'変更後の路線名'}) as HTMLInputElement).value).toBe('新本線');
 expect(invoke).toHaveBeenCalledWith('rename_route',{routeId:'6',name:'新本線',expectedName:'本線'});
 fireEvent.submit(form);await screen.findByRole('button',{name:'編集: 新本線'});
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='save_planned_route'||c.includes('service_path'))).toBe(false);
});
test('world switching warns for pending rename and discards it only after confirmed successful load',async()=>{
 await setup();
 fireEvent.click(screen.getByRole('button',{name:'読込'}));
 const dialog=await screen.findByRole('dialog',{name:'ワールドを選択'});
 fireEvent.click(within(dialog).getByRole('button',{name:'別ワールド'}));
 expect(screen.getByRole('button',{name:'読込を続ける'})).toBeTruthy();
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='load_save_slot')).toBe(false);
 fireEvent.click(screen.getByRole('button',{name:'戻る'}));
 expect((screen.getByRole('textbox',{name:'変更後の路線名'}) as HTMLInputElement).value).toBe('新本線');
 fireEvent.click(within(dialog).getByRole('button',{name:'別ワールド'}));
 fireEvent.click(screen.getByRole('button',{name:'読込を続ける'}));
 await screen.findByRole('button',{name:'編集: 別世界線'});
 expect(screen.queryByRole('textbox',{name:'変更後の路線名'})).toBeNull();
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='rename_route')).toBe(false);
});

test('re-click deselects a saved route without opening a new route or touching native state',async()=>{
 await setup();
 fireEvent.click(screen.getByRole('button',{name:'取消'}));
 const route=screen.getByRole('button',{name:'編集: 本線'});
 fireEvent.click(route);
 expect(route.getAttribute('aria-pressed')).toBe('true');
 expect(screen.getByRole('region',{name:'保存済み路線の経路'})).toBeTruthy();
 const editMap=screen.getByRole('button',{name:'地図で路線を編集'});
 fireEvent.click(editMap);
 expect((route as HTMLButtonElement).disabled).toBe(true);
 fireEvent.click(screen.getByRole('button',{name:'キャンセル'}));
 expect(route.getAttribute('aria-pressed')).toBe('true');

 const before=vi.mocked(invoke).mock.calls.length;
 fireEvent.click(route);
 expect(route.getAttribute('aria-pressed')).toBe('false');
 expect(screen.queryByRole('region',{name:'保存済み路線の経路'})).toBeNull();
 expect(screen.queryByRole('button',{name:'地図で路線を編集'})).toBeNull();
 expect(screen.queryByRole('button',{name:'編集終了'})).toBeNull();
 expect(vi.mocked(invoke).mock.calls.slice(before)).toEqual([]);

 fireEvent.click(screen.getByRole('button',{name:'新規路線'}));
 expect(screen.getByRole('button',{name:'編集終了'})).toBeTruthy();
 expect(route.getAttribute('aria-pressed')).toBe('false');
});
