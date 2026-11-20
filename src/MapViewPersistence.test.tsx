// @vitest-environment jsdom
import React from 'react';
import {panMap,sampleCamera} from './mapTestNavigation';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
import {DEFAULT_CAMERA,panCamera} from './map';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
const base={next_id:'3',players:[{id:'1',name:'Player'}],companies:[{id:'2',owner:'1',name:'Railway'}],stations:[],tracks:[],routes:[],services:[]};
const view={latitude:34.7,longitude:135.5,zoom:16};
async function start(){
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='list_save_slots')return {active_slot_id:'test-slot',slots:[]};if(command==='engine_info')return {name:'Rosenfu',version:'view-test'};
  if(command==='world_snapshot')return {...base,view_state:view};
  if(command==='autosave_status')return {available:false};
  if(command==='save_world')return {saved:true};
  if(command==='load_save_slot')return {...base,view_state:view};
  throw Error('unexpected '+command);
 });
 render(<App/>);await screen.findByText(/view-test/);fireEvent.click(screen.getByRole('button',{name:'自作サンプル（オフライン）'}));await waitFor(()=>expect(invoke).toHaveBeenCalledWith('autosave_status',{slotId:'test-slot'}));
}
const latitude=()=>Number(sampleCamera().latitude.toFixed(5));
test('restores saved center and zoom on startup, includes current view in manual save and restores on reload',async()=>{
 await start();expect(latitude()).toBe(34.7);expect(screen.getByText('ZL 16 / Web Mercator')).toBeTruthy();
 panMap(100,80);
 fireEvent.click(screen.getByRole('button',{name:'保存'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('save_world',expect.objectContaining({viewState:panCamera(view,100,80)})));
 fireEvent.click(screen.getByRole('button',{name:'読込'}));
 await waitFor(()=>expect(latitude()).toBe(34.7));
});
test('failed load leaves view intact; loading legacy world resets view rather than leaking previous world camera',async()=>{
 await start();vi.mocked(invoke).mockRejectedValueOnce(Error('read failed'));
 fireEvent.click(screen.getByRole('button',{name:'読込'}));await screen.findByText(/read failed/);expect(latitude()).toBe(34.7);
 vi.mocked(invoke).mockResolvedValueOnce(base);
 fireEvent.click(screen.getByRole('button',{name:'読込'}));await waitFor(()=>expect(latitude()).toBe(DEFAULT_CAMERA.latitude));
});
