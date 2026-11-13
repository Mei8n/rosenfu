// @vitest-environment jsdom
import React from 'react';
import {cleanup,fireEvent,render,screen,waitFor,within} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
const world={next_id:'4',players:[{id:'1',name:'Player'}],companies:[{id:'2',owner:'1',name:'Railway'}],stations:[{id:'3',owner:'2',name:'既存駅',latitude:35.67,longitude:139.75}],tracks:[],routes:[],services:[]};
async function start(){vi.mocked(isTauri).mockReturnValue(true);vi.mocked(invoke).mockImplementation(async command=>command==='engine_info'?{name:'Rosenfu',version:'rename-test'}:world);const view=render(<App/>);await screen.findByText(/rename-test/);fireEvent.click(screen.getByRole('tab',{name:'駅'}));fireEvent.click(screen.getByRole('button',{name:/^既存駅 /}));return {view,editor:within(view.container.querySelector('.equipment-editor') as HTMLElement)};}
test('double click and accessible pen focus/select existing editor; Escape cancels without mutation or station undo',async()=>{
 const {view,editor}=await start();const input=editor.getByLabelText('選択駅の名前') as HTMLInputElement;
 fireEvent.doubleClick(view.container.querySelector('.station-detail-name strong')!);expect(document.activeElement).toBe(input);expect(input.selectionStart).toBe(0);expect(input.selectionEnd).toBe(3);
 fireEvent.change(input,{target:{value:'下書き'}});fireEvent.keyDown(input,{key:'z',ctrlKey:true});expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='delete_station')).toBe(false);
 fireEvent.keyDown(input,{key:'Escape'});expect(input.value).toBe('既存駅');expect(document.activeElement).toBe(editor.getByRole('button',{name:'選択駅の名前を編集'}));
 fireEvent.click(editor.getByRole('button',{name:'選択駅の名前を編集'}));expect(document.activeElement).toBe(input);expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='rename_station')).toBe(false);
});
test('native form submission retains failed name, busy prevents entry, retry uses same rename API',async()=>{
 const {view,editor}=await start();const input=editor.getByLabelText('選択駅の名前') as HTMLInputElement;fireEvent.change(input,{target:{value:'新しい駅'}});
 let reject!:(reason:Error)=>void;vi.mocked(invoke).mockImplementationOnce(()=>new Promise((_resolve,fail)=>{reject=fail;}));
 // Enter in a text input submits this same native form in the browser.
 fireEvent.submit(input.closest('form')!);expect(invoke).toHaveBeenCalledWith('rename_station',{stationId:'3',name:'新しい駅'});
 const pen=editor.getByRole('button',{name:'選択駅の名前を編集'}) as HTMLButtonElement;expect(pen.disabled).toBe(true);input.blur();fireEvent.doubleClick(view.container.querySelector('.station-detail-name strong')!);expect(document.activeElement).not.toBe(input);
 fireEvent.keyDown(input,{key:'Escape'});expect(input.value).toBe('新しい駅');reject(Error('rename rejected'));await screen.findByText(/rename rejected/);await waitFor(()=>expect(pen.disabled).toBe(false));expect(input.value).toBe('新しい駅');
 vi.mocked(invoke).mockResolvedValueOnce({...world,stations:[{...world.stations[0],name:'新しい駅'}]});fireEvent.submit(input.closest('form')!);await screen.findByText(/駅名を変更しました/);expect(view.container.querySelector('.station-detail-name strong')?.textContent).toBe('新しい駅');
});

test('Escape during IME composition preserves draft and focus',async()=>{
 const {editor}=await start();const input=editor.getByLabelText('選択駅の名前') as HTMLInputElement;fireEvent.click(editor.getByRole('button',{name:'選択駅の名前を編集'}));fireEvent.change(input,{target:{value:'変換途中'}});
 expect(fireEvent.keyDown(input,{key:'Escape',isComposing:true})).toBe(true);expect(input.value).toBe('変換途中');expect(document.activeElement).toBe(input);
 expect(fireEvent.keyDown(input,{key:'Escape',keyCode:229})).toBe(true);expect(input.value).toBe('変換途中');
 fireEvent.keyDown(input,{key:'Escape'});expect(input.value).toBe('既存駅');expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='rename_station')).toBe(false);
});
