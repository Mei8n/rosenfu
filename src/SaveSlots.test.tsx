// @vitest-environment jsdom
import React from 'react';
import {settledMapClick} from './testMapClick';
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen,within,waitFor} from '@testing-library/react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
const world={next_id:'3',players:[{id:'1',name:'Player'}],companies:[{id:'2',owner:'1',name:'現在会社'}],stations:[],routes:[],services:[],route_paths:[],service_paths:[]};
const alignedWorld={...world,alignments:[{id:'50',owner:'2',name:'試験線',points:[{latitude:35.67504,longitude:139.72},{latitude:35.67504,longitude:139.78}]}],station_placements:[]};
async function setup(withAlignment=false,active:string|null='one'){
 const state={active,failLoad:false,failSave:false};
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'slots-test'};
  if(command==='world_snapshot')return withAlignment?alignedWorld:world;
  if(command==='list_save_slots')return {active_slot_id:state.active,slots:[{id:'one',name:'第一ワールド',loadable:true},{id:'two',name:'第二ワールド',loadable:true},{id:'old',name:'旧保存',loadable:false,error:'非対応の保存形式'}]};
  if(command==='autosave_status')return {available:false};
  if(command==='load_save_slot'){if(state.failLoad)throw Error('読込失敗');state.active=(args as any).slotId;return {...world,companies:[{...world.companies[0],name:'読込会社'}]};}
  if(command==='create_new_world'){state.active=null;return {...world,companies:[{...world.companies[0],name:(args as any).companyName}]};}
  if(command==='save_world_as'){if(state.failSave)throw Error('保存失敗');state.active='copy';return {saved:true};}
  if(command==='save_world'){state.active='first-save';return {saved:true};}
  throw Error(command);
 });render(<App/>);await screen.findByText(/slots-test/);fireEvent.click(screen.getByRole('tab',{name:'セーブデータ'}));await screen.findByRole('button',{name:active?'第一ワールド（現在）':'第一ワールド'});return state;
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
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='save_world'||c==='confirm_manual_save')).toBe(false);
 expect(invoke).toHaveBeenCalledWith('save_world_as',expect.objectContaining({name:'新会社'}));
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
 await setup(true);
 const sameId={...world,stations:[{id:'7',owner:'2',name:'別ワールドの駅',latitude:35.67,longitude:139.75}]};
 let rejectLoad=true;const previous=vi.mocked(invoke).getMockImplementation()!;
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command==='build_plan')return {...sameId,stations:[{...sameId.stations[0],name:'旧ワールドの駅'}]};
  if(command==='load_save_slot'){if(rejectLoad){rejectLoad=false;throw Error('切替失敗');}return sameId;}
  return previous(command,args);
 });
 fireEvent.click(screen.getByRole('tab',{name:'建設'}));fireEvent.click(screen.getByRole('tab',{name:'駅設置'}));
 const map=screen.getByRole('img',{name:'駅配置用OpenStreetMap'});map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 await settledMapClick(map,{clientX:250,clientY:150});await screen.findByText(/を計画に追加しました/);
 fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));await screen.findByText('計画駅を建設しました。');
 fireEvent.click(screen.getByRole('tab',{name:'駅'}));
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

test('world name dialogs disable saved-info autocomplete without preventing input or submission',async()=>{
 await setup();
 for(const title of ['新しいワールド','別のセーブとして保存']){
  fireEvent.click(screen.getByRole('button',{name:title}));
  const modal=within(screen.getByRole('dialog',{name:title}));
  const input=modal.getByLabelText('名前') as HTMLInputElement;
  expect(input.autocomplete).toBe('off');
  expect(input.form?.getAttribute('autocomplete')).toBe('off');
  fireEvent.change(input,{target:{value:'試験セーブ'}});
  expect(input.value).toBe('試験セーブ');
  fireEvent.click(modal.getByRole('button',{name:'キャンセル'}));
 }
});

test('failed initial save detaches the old slot and leaves the new world editable',async()=>{
 const state=await setup();state.failSave=true;
 fireEvent.click(screen.getByRole('button',{name:'新しいワールド'}));
 const modal=within(screen.getByRole('dialog',{name:'新しいワールド'}));
 fireEvent.change(modal.getByLabelText('名前'),{target:{value:'未保存の新世界'}});
 fireEvent.click(modal.getByRole('button',{name:'作成'}));
 await screen.findByText(/処理できませんでした: 保存失敗/);
 expect(screen.queryByRole('button',{name:'第一ワールド（現在）'})).toBeNull();
 expect((document.getElementById('company-name') as HTMLInputElement).value).toBe('未保存の新世界');
 fireEvent.click(screen.getByRole('button',{name:'保存'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('save_world',expect.anything()));
});


test('Ctrl+S overwrites only the active slot once and announces success only after saving',async()=>{
 await setup();
 const original=vi.mocked(invoke).getMockImplementation()!;
 let finish!:(result:{saved:boolean})=>void;
 const saved=new Promise<{saved:boolean}>(resolve=>{finish=resolve;});
 vi.mocked(invoke).mockImplementation((command,args)=>command==='save_world'?saved:original(command,args));
 fireEvent.click(screen.getByRole('tab',{name:'建設'}));
 fireEvent.keyDown(window,{key:'s',ctrlKey:true,repeat:true});
 expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==='save_world')).toHaveLength(0);
 fireEvent.keyDown(window,{key:'s',ctrlKey:true});
 fireEvent.keyDown(window,{key:'s',ctrlKey:true});
 expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==='save_world')).toEqual([
  ['save_world',{slotId:'one',constructionPlans:expect.any(Object),viewState:expect.any(Object)}],
 ]);
 expect(screen.queryByRole('dialog')).toBeNull();
 expect(screen.queryByText(/ワールドと未着工計画を保存しました/)).toBeNull();
 await act(async()=>finish({saved:true}));
 expect(screen.getByText(/ワールドと未着工計画を保存しました/)).toBeTruthy();
 expect(screen.queryByRole('dialog')).toBeNull();
});

test('Ctrl+S has no fallback write or dialog when there is no current slot',async()=>{
 await setup(false,null);
 fireEvent.keyDown(window,{key:'s',ctrlKey:true});
 expect(screen.getByText(/上書きするセーブがありません/)).toBeTruthy();
 expect(screen.queryByRole('dialog')).toBeNull();
 expect(vi.mocked(invoke).mock.calls.some(([command])=>/^save_world/.test(command))).toBe(false);
});

test('Ctrl+S ignores input composition and save dialogs; a stale slot fails without success',async()=>{
 await setup();
 fireEvent.keyDown(window,{key:'s',ctrlKey:true,isComposing:true});
 fireEvent.click(screen.getByRole('button',{name:'別のセーブとして保存'}));
 fireEvent.keyDown(window,{key:'s',ctrlKey:true});
 expect(vi.mocked(invoke).mock.calls.some(([command])=>/^save_world/.test(command))).toBe(false);
 const modal=within(screen.getByRole('dialog',{name:'別のセーブとして保存'}));
 fireEvent.click(modal.getByRole('button',{name:'キャンセル'}));
 const original=vi.mocked(invoke).getMockImplementation()!;
 vi.mocked(invoke).mockImplementation((command,args)=>command==='save_world'?Promise.reject(Error('ワールドが切り替わったため、以前の操作を中止しました。')):original(command,args));
 fireEvent.keyDown(window,{key:'s',ctrlKey:true});
 await screen.findByText(/ワールドが切り替わったため/);
 expect(screen.queryByText(/ワールドと未着工計画を保存しました/)).toBeNull();
 expect(screen.queryByRole('dialog')).toBeNull();
});

test('Cmd+S shares the shortcut and does not bypass an unapplied draft',async()=>{
 await setup();
 const original=vi.mocked(invoke).getMockImplementation()!;
 vi.mocked(invoke).mockImplementation(async(command,args)=>command==='save_world'?{saved:true}:original(command,args));
 const name=document.getElementById('company-name')!;
 fireEvent.change(name,{target:{value:'未適用'}});
 fireEvent.keyDown(window,{key:'s',metaKey:true});
 expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='save_world')).toBe(false);
 fireEvent.change(name,{target:{value:'現在会社'}});
 fireEvent.keyDown(window,{key:'s',metaKey:true});
 await screen.findByText(/ワールドと未着工計画を保存しました/);
 expect(invoke).toHaveBeenCalledWith('save_world',expect.objectContaining({slotId:'one'}));
});
