// @vitest-environment jsdom
import React from 'react';
import {act,cleanup,fireEvent,render,screen} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
// Test-only Node built-in: the app deliberately has no Node type dependency.
// @ts-expect-error Node fs is available in Vitest, not in the browser type environment.
import {readFileSync} from 'node:fs';
const styles=readFileSync('src/style.css','utf8');
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();localStorage.clear();document.head.querySelectorAll('style[data-workspace-test]').forEach(style=>style.remove());});
test('construction workspace collapses and reopens repeatedly without losing its plan',async()=>{
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'collapse-test'};
  if(command==='world_snapshot')return {next_id:'3',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
  throw Error('unexpected '+command);
 });
 const style=document.createElement('style');style.dataset.workspaceTest='true';style.textContent=styles;document.head.append(style);
 await act(async()=>{render(<App/>);});
 const panel=document.getElementById('workspace-panel-construction')!;
 fireEvent.change(screen.getByRole('textbox',{name:'計画名（任意）'}),{target:{value:'保持する計画'}});
 for(const tool of ['線路','駅設置','削除']){
  fireEvent.click(screen.getByRole('tab',{name:tool}));
  expect(getComputedStyle(panel).display).not.toBe('none');
  fireEvent.click(screen.getByRole('button',{name:'操作パネルを畳む'}));
  expect(getComputedStyle(panel).display).toBe('none');
  fireEvent.click(screen.getByRole('button',{name:'操作パネルを開く'}));
  expect(getComputedStyle(panel).display).not.toBe('none');
  expect((screen.getByRole('textbox',{name:'計画名（任意）'}) as HTMLInputElement).value).toBe('保持する計画');
 }
 fireEvent.click(screen.getByRole('button',{name:'操作パネルを畳む'}));
 fireEvent.click(screen.getByRole('tab',{name:'財務'}));
 expect(screen.getByRole('button',{name:'操作パネルを畳む'})).toBeTruthy();
 fireEvent.click(screen.getByRole('tab',{name:'建設'}));
 expect(getComputedStyle(panel).display).not.toBe('none');
 expect((screen.getByRole('textbox',{name:'計画名（任意）'}) as HTMLInputElement).value).toBe('保持する計画');
});
