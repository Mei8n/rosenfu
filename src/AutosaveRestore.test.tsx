// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen,waitFor,within} from '@testing-library/react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
const empty={next_id:'3',players:[{id:'1',name:'Player'}],companies:[{id:'2',owner:'1',name:'Railway'}],stations:[],tracks:[],routes:[],services:[]};
test('restore asks before discarding plans, failed restore retains them, successful restore stays manually unsaved',async()=>{
 vi.mocked(isTauri).mockReturnValue(true);let fail=true;vi.mocked(invoke).mockImplementation(async command=>{if(command==='engine_info')return {version:'auto-test'};if(command==='world_snapshot')return empty;if(command==='autosave_status')return {available:true};if(command==='load_autosave'){if(fail)throw Error('bad autosave');return {...empty,next_id:'4',stations:[{id:'3',owner:'2',name:'復元駅',latitude:35.67,longitude:139.75}]};}if(command==='save_world')return {saved:true};throw Error(command);});
 const view=render(<App/>);fireEvent.click(screen.getByRole('button',{name:'自作サンプル（オフライン）'}));await screen.findByText(/auto-test/);await waitFor(()=>expect(screen.queryByText("ワールドを更新しています。")).toBeNull());
 const map=screen.getByRole('img',{name:'駅配置用サンプル地図'});map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});fireEvent.click(map,{clientX:180,clientY:130});
 fireEvent.click(screen.getByRole('tab',{name:'セーブデータ'}));const panel=within(view.container.querySelector('#workspace-panel-saves')! as HTMLElement);await waitFor(()=>expect((panel.getByRole('button',{name:'自動保存から復元'}) as HTMLButtonElement).disabled).toBe(false));
 expect(panel.getByText('自動保存間隔：2分（固定・実時間）')).toBeTruthy();expect(panel.queryByRole('combobox',{name:'自動保存間隔'})).toBeNull();
 fireEvent.click(panel.getByRole('button',{name:'自動保存から復元'}));expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='load_autosave')).toBe(false);fireEvent.click(screen.getByRole('button',{name:'戻る'}));expect(map.querySelectorAll('.draft-anchor')).toHaveLength(1);
 fireEvent.click(panel.getByRole('button',{name:'自動保存から復元'}));fireEvent.click(screen.getByRole('button',{name:'読込を続ける'}));await screen.findByText(/bad autosave/);expect(map.querySelectorAll('.draft-anchor')).toHaveLength(1);
 fail=false;fireEvent.click(panel.getByRole('button',{name:'自動保存から復元'}));fireEvent.click(screen.getByRole('button',{name:'読込を続ける'}));await screen.findByText(/自動保存から復元しました/);expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);expect(screen.getByText('未保存の変更あり')).toBeTruthy();expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='save_world')).toBe(false);
});
test('autosave restore is available before new company setup',async()=>{
 vi.mocked(isTauri).mockReturnValue(true);vi.mocked(invoke).mockImplementation(async command=>{if(command==='engine_info')return {version:'auto-start',company_setup_required:true};if(command==='autosave_status')return {available:true};return empty;});render(<App/>);
 const dialog=within(await screen.findByRole('dialog'));fireEvent.click(await dialog.findByRole('button',{name:'自動保存から復元'}));await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());expect(invoke).toHaveBeenCalledWith('load_autosave');expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='rename_company')).toBe(false);
});
