import './testConstructionEnvironment';
// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen,within,waitFor} from '@testing-library/react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
const world={next_id:'3',players:[{id:'1',name:'Player'}],companies:[{id:'2',owner:'1',name:'現在会社'}],stations:[],routes:[],services:[],route_paths:[],service_paths:[]};
async function setup(){
 const state={active:'one' as string|null,failLoad:false,failSave:false};
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'slots-test'};
  if(command==='world_snapshot')return world;
  if(command==='list_save_slots')return {active_slot_id:state.active,slots:[{id:'one',name:'第一ワールド',loadable:true},{id:'two',name:'第二ワールド',loadable:true},{id:'old',name:'旧保存',loadable:false,error:'非対応の保存形式'}]};
  if(command==='autosave_status')return {available:false};
  if(command==='load_save_slot'){if(state.failLoad)throw Error('読込失敗');state.active=(args as any).slotId;return {...world,companies:[{...world.companies[0],name:'読込会社'}]};}
  if(command==='create_new_world'){state.active=null;return {...world,companies:[{...world.companies[0],name:(args as any).companyName}]};}
  if(command==='save_world_as'){if(state.failSave)throw Error('保存失敗');state.active='copy';return {saved:true};}
  if(command==='save_world'){state.active='first-save';return {saved:true};}
  throw Error(command);
 });render(<App/>);await screen.findByText(/slots-test/);fireEvent.click(screen.getByRole('tab',{name:'セーブデータ'}));await screen.findByRole('button',{name:'第一ワールド（現在）'});return state;
}
test('slot load failure retains world and selection; incompatible slots cannot load',async()=>{
 const state=await setup();expect((screen.getByRole('button',{name:'旧保存'}) as HTMLButtonElement).disabled).toBe(true);
 state.failLoad=true;fireEvent.click(screen.getByRole('button',{name:'第二ワールド'}));await screen.findByText(/処理できませんでした: 読込失敗/);
 expect(screen.getByRole('button',{name:'第一ワールド（現在）'})).toBeTruthy();
 expect((document.getElementById('company-name') as HTMLInputElement).value).toBe('現在会社');
 state.failLoad=false;fireEvent.click(screen.getByRole('button',{name:'第二ワールド'}));await screen.findByRole('button',{name:'第二ワールド（現在）'});
 expect((document.getElementById('company-name') as HTMLInputElement).value).toBe('読込会社');
});
test('new world asks before discarding edits and never overwrites a slot',async()=>{
 await setup();fireEvent.change(document.getElementById('company-name')!,{target:{value:'未適用会社'}});
 fireEvent.click(screen.getByRole('button',{name:'新しいワールド'}));
 const modal=within(screen.getByRole('dialog',{name:'新しいワールド'}));fireEvent.change(modal.getByLabelText('名前'),{target:{value:'新会社'}});fireEvent.click(modal.getByRole('button',{name:'作成'}));
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='create_new_world')).toBe(false);
 fireEvent.click(screen.getByRole('button',{name:'戻る'}));expect((document.getElementById('company-name') as HTMLInputElement).value).toBe('未適用会社');
 fireEvent.click(modal.getByRole('button',{name:'作成'}));fireEvent.click(screen.getByRole('button',{name:'変更を破棄して作成'}));await screen.findByText(/新しいワールドを作成しました/);
 expect(invoke).toHaveBeenCalledWith('create_new_world',{companyName:'新会社'});
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='save_world'||c==='confirm_manual_save'||c==='save_world_as')).toBe(false);
 expect(screen.getByText('初回保存後に自動保存を開始します。')).toBeTruthy();
});
test('save as creates a separate slot, keeps name on failure and never calls overwrite',async()=>{
 const state=await setup();fireEvent.click(screen.getByRole('button',{name:'別のセーブとして保存'}));const modal=within(screen.getByRole('dialog',{name:'別のセーブとして保存'}));
 fireEvent.change(modal.getByLabelText('名前'),{target:{value:'別ワールド'}});state.failSave=true;fireEvent.click(modal.getByRole('button',{name:'保存'}));await waitFor(()=>expect(modal.getByRole('alert').textContent).toContain('保存失敗'));
 expect((modal.getByLabelText('名前') as HTMLInputElement).value).toBe('別ワールド');
 state.failSave=false;fireEvent.click(modal.getByRole('button',{name:'保存'}));await screen.findByText(/元の保存データは変更していません/);
 expect(invoke).toHaveBeenCalledWith('save_world_as',expect.objectContaining({name:'別ワールド',constructionPlans:expect.any(Object),viewState:expect.any(Object)}));
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='confirm_manual_save'||c==='save_world')).toBe(false);
});
test('switching worlds clears deletion confirmation and station undo even when IDs coincide',async()=>{
 await setup();
 const sameId={...world,stations:[{id:'7',owner:'2',name:'別ワールドの駅',latitude:35.67,longitude:139.75}]};
 let rejectLoad=true;const previous=vi.mocked(invoke).getMockImplementation()!;
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command==='add_station')return {...sameId,stations:[{...sameId.stations[0],name:'旧ワールドの駅'}]};
  if(command==='load_save_slot'){if(rejectLoad){rejectLoad=false;throw Error('切替失敗');}return sameId;}
  return previous(command,args);
 });
 fireEvent.click(screen.getByRole('tab',{name:'駅'}));fireEvent.click(screen.getByRole('button',{name:'指定座標に配置'}));await screen.findByText('新駅を配置しました。');
 fireEvent.click(document.querySelector('.station-entry')!);fireEvent.click(screen.getByRole('button',{name:'選択駅を削除'}));expect(screen.getByRole('button',{name:'削除を確定'})).toBeTruthy();
 fireEvent.click(screen.getByRole('tab',{name:'セーブデータ'}));fireEvent.click(screen.getByRole('button',{name:'第二ワールド'}));fireEvent.click(screen.getByRole('button',{name:'読込を続ける'}));await screen.findByText(/処理できませんでした: 切替失敗/);
 expect(screen.getByRole('button',{name:'削除を確定'})).toBeTruthy();expect(document.querySelector('.station-entry')?.textContent).toContain('旧ワールドの駅');
 fireEvent.click(screen.getByRole('button',{name:'第二ワールド'}));fireEvent.click(screen.getByRole('button',{name:'読込を続ける'}));await screen.findByText('保存したワールドを読み込みました。');
 expect(screen.queryByRole('button',{name:'削除を確定'})).toBeNull();
 fireEvent.click(screen.getByRole('tab',{name:'建設'}));fireEvent.click(screen.getByRole('tab',{name:'駅設置'}));
 fireEvent.keyDown(window,{key:'z',ctrlKey:true});fireEvent.keyDown(window,{key:'z',ctrlKey:true,shiftKey:true});
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='delete_station'||c==='restore_deleted_alignment')).toBe(false);
 expect(document.querySelector('.station-entry')?.textContent).toContain('別ワールドの駅');
});
test('unexpected overwrite response is rejected without confirming or clearing edits',async()=>{
 await setup();const previous=vi.mocked(invoke).getMockImplementation()!;
 vi.mocked(invoke).mockImplementation(async(command,args)=>command==='save_world'?{saved:false,replacement_required:{token:'old',company_name:'別会社'}}:previous(command,args));
 fireEvent.click(screen.getByRole('button',{name:'保存'}));await screen.findByText(/既存セーブの置き換えは行いません/);
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='confirm_manual_save')).toBe(false);
 expect(screen.queryByRole('dialog',{name:'保存済みのワールドを置き換えますか？'})).toBeNull();
});
