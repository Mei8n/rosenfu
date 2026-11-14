// @vitest-environment jsdom
import React from 'react';
import {afterEach,beforeEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
beforeEach(()=>{
 HTMLDialogElement.prototype.showModal=vi.fn(function(this:HTMLDialogElement){this.open=true;});
 HTMLDialogElement.prototype.close=vi.fn(function(this:HTMLDialogElement){this.open=false;});
});
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
const world={next_id:'3',players:[{id:'1',name:'Player'}],companies:[{id:'2',owner:'1',name:'New Railway'}],stations:[],tracks:[],routes:[],services:[]};
async function start(){
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'replacement-test'};
  if(command==='world_snapshot')return world;
  if(command==='autosave_status')return {available:false};
  if(command==='save_world')return {saved:false,replacement_required:{token:'request-1',company_name:'Old Railway'}};
  if(command==='cancel_manual_save')return;
  if(command==='confirm_manual_save')return {saved:true};
  throw Error('unexpected '+command);
 });
 render(<App/>);await screen.findByText(/replacement-test/);
 fireEvent.click(screen.getByRole('button',{name:'保存'}));
 await screen.findByRole('dialog',{name:'保存済みのワールドを置き換えますか？'});
}
test('new session cannot replace silently; safe cancellation keeps editor and never confirms',async()=>{
 await start();expect(screen.getByText(/Old Railway.*New Railway/)).toBeTruthy();
 expect(screen.queryByText('ワールドと未着工計画を保存しました。')).toBeNull();
 expect(document.activeElement).toBe(screen.getByRole('button',{name:'置き換えず編集に戻る'}));
 fireEvent.click(screen.getByRole('button',{name:'置き換えず編集に戻る'}));
 await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());
 expect(invoke).toHaveBeenCalledWith('cancel_manual_save',{token:'request-1'});
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='confirm_manual_save'||c==='load_world')).toBe(false);
});
test('explicit replacement sends token and current plans/view once, then reports success',async()=>{
 await start();const button=screen.getByRole('button',{name:'バックアップして置き換える'});
 fireEvent.click(button);fireEvent.click(button);
 await screen.findByText('ワールドと未着工計画を保存しました。');
 expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==='confirm_manual_save')).toHaveLength(1);
 expect(invoke).toHaveBeenCalledWith('confirm_manual_save',expect.objectContaining({token:'request-1',constructionPlans:expect.any(Object),viewState:expect.any(Object)}));
 expect(screen.queryByRole('dialog')).toBeNull();
});
test('stale confirmation preserves editor, does not reload or claim success, and can cancel',async()=>{
 await start();vi.mocked(invoke).mockRejectedValueOnce(Error('changed after confirmation'));
 fireEvent.click(screen.getByRole('button',{name:'バックアップして置き換える'}));
 await screen.findByText(/編集中の内容は保持しています/);
 expect((screen.getByRole('button',{name:'バックアップして置き換える'}) as HTMLButtonElement).disabled).toBe(true);
 expect(screen.queryByText('ワールドと未着工計画を保存しました。')).toBeNull();
 fireEvent.click(screen.getByRole('button',{name:'置き換えず編集に戻る'}));await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='load_world')).toBe(false);
});
test('Escape cancels replacement rather than confirming it',async()=>{
 await start();fireEvent(screen.getByRole('dialog'),new Event('cancel',{cancelable:true}));
 await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='confirm_manual_save')).toBe(false);
});
test('ordinary save conflict never offers a forced overwrite or tells the user to reload',async()=>{
 await start();fireEvent.click(screen.getByRole('button',{name:'置き換えず編集に戻る'}));await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());
 vi.mocked(invoke).mockRejectedValueOnce(Error('save file changed since it was observed; reload before saving'));
 fireEvent.click(screen.getByRole('button',{name:'保存'}));
 await screen.findByText(/未保存のまま読込や終了をせず/);
 expect(screen.queryByRole('dialog')).toBeNull();
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='confirm_manual_save'||c==='load_world')).toBe(false);
});
