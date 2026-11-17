// @vitest-environment jsdom
import React from 'react';
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
import {settledMapClick} from './testMapClick';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
window.PointerEvent=MouseEvent as typeof PointerEvent;
afterEach(()=>{cleanup();vi.resetAllMocks();localStorage.clear();});
const world={next_id:'3',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
async function start(fail:boolean){
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
 if(command==='engine_info')return {name:'Rosenfu',version:'boundary-test'};
 if(command==='world_snapshot')return structuredClone(world);
 if(command==='preview_construction_plan')return {validationError:null};
 if(command==='list_save_slots')return {active_slot_id:'test',slots:[]};
 if(command==='autosave_status')return {available:false};
 if(command==='build_alignment'){if(fail)throw Error('build failed');return {...world,next_id:'4',alignments:[{id:'3',owner:'2',name:(args as any).name,points:(args as any).points,track_type:'normal'}]};}
 throw Error(command);
 });
 await act(async()=>{render(<App/>);});
 fireEvent.click(screen.getByRole('button',{name:'自作サンプル（オフライン）'}));
 const map=screen.getByRole('img',{name:'駅配置用サンプル地図'});
 map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 return map;
}
test.each([false,true])('construction finalizes history only on success (failure=%s)',async fail=>{
 const map=await start(fail);
 const click=async(x:number,y:number)=>settledMapClick(map,{clientX:x,clientY:y,detail:1});
 await click(100,200);await click(220,200);await click(220,200);
 fireEvent.click(screen.getByRole('button',{name:'別の線路計画を追加'}));
 await click(100,80);await click(220,80);await click(220,80);
 const plansBefore=map.querySelectorAll('.draft-path').length;
 fireEvent.click(screen.getByRole('button',{name:/計画 1 を編集/}));
 await act(async()=>{fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));});
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('build_alignment',expect.anything()));
 if(fail){
  await screen.findByText(/build failed/);
  expect(map.querySelectorAll('.draft-path').length).toBe(plansBefore);
  expect((screen.getByRole('button',{name:'元に戻す'}) as HTMLButtonElement).disabled).toBe(false);
 }else{
  await screen.findByText(/線路を建設しました/);
  expect(map.querySelectorAll('.draft-path').length).toBeGreaterThan(0);
  const remaining=map.querySelector('.draft-path')?.getAttribute('d');
  fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
  expect(map.querySelector('.draft-path')?.getAttribute('d')).toBe(remaining);
  expect((screen.getByRole('button',{name:'元に戻す'}) as HTMLButtonElement).disabled).toBe(true);
  expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='undo_world_edit'||c==='delete_alignment')).toBe(false);
 }
});
