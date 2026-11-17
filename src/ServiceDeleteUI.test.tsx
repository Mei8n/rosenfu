// @vitest-environment jsdom
import React from 'react';
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
const segment={kind:'alignment',alignment_id:'9',from:'3',to:'5'};
const path={service_id:'6',next_leg_key:'2',legs:[{key:'1',segment}],stops:[{kind:'start'},{kind:'on_leg',leg_key:'1',station_id:'5'}]};
const base={next_id:'10',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:['3','4','5'].map((id,i)=>({id,owner:'2',name:['起点','途中','終点'][i],latitude:35,longitude:139+i*.01})),routes:[],route_paths:[],services:[{id:'6',owner:'2',name:'普通',start:'3'}],service_paths:[path],service_route_bindings:[{service_id:'6',binding:{kind:'independent'}}],service_appearances:[{service_id:'6',color:'#123456',shape:'circle',symbol:'普'}],station_placements:['3','4','5'].map((station_id,i)=>({station_id,alignment_id:'9',position:i/2,length_m:100,width_m:12}))};
import {emptyConstructionPlans} from './constructionPlans';
import {settledMapClick} from './testMapClick';
test.each([false,true])('service deletion Undo/Redo preserves unsaved plans (deferred=%s)',async(deferred)=>{
 const before={...structuredClone(base),construction_plans:emptyConstructionPlans()};
 const after={...before,services:[],service_paths:[],service_route_bindings:[],service_appearances:[]};
 let finishMutation:(()=>void)|undefined;
 const mutationCount=()=>vi.mocked(invoke).mock.calls.filter(([command])=>command==='delete_service'||command==='undo_world_edit'||command==='redo_world_edit').length;
 // A visible world update is not a promise that perform() has released its busy
 // guard. Flush the whole operation before sending the next keyboard gesture.
 async function mutate(gesture:()=>void,command:string){
  const count=mutationCount();
  await act(async()=>{gesture();});
  expect(mutationCount()).toBe(count+1);
  expect(vi.mocked(invoke).mock.calls.filter(([name])=>name==='delete_service'||name==='undo_world_edit'||name==='redo_world_edit').at(-1)?.[0]).toBe(command);
  if(deferred){
   expect(finishMutation).toBeTypeOf('function');
   // In-flight input must not run another mutation or undo a construction plan.
   const draftBefore=Array.from(document.querySelectorAll('.draft-path')).map(p=>p.getAttribute('d'));
   fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
   expect(mutationCount()).toBe(count+1);
   expect(Array.from(document.querySelectorAll('.draft-path')).map(p=>p.getAttribute('d'))).toEqual(draftBefore);
   await act(async()=>{finishMutation!();finishMutation=undefined;});
  }
 }
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async (command,args)=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'delete-ui-test'};
  if(command==='world_snapshot')return structuredClone(before);
  if(command==='undo_world_edit'||command==='redo_world_edit'||command==='delete_service'){
   const expected=(args as any)?.expectedWorld;
   const result={...structuredClone(command==='undo_world_edit'?before:after),...(expected?{construction_plans:expected.construction_plans,view_state:expected.view_state}:{})};
   return deferred?new Promise(resolve=>{finishMutation=()=>resolve(result);}):result;
  }
  if(command==='list_save_slots')return {active_slot_id:'slot',slots:[]};
  if(command==='autosave_status')return {available:false};
  if(command==='preview_construction_plan')return {validationError:null};
  throw Error(command);
 });
 render(<App/>);await screen.findByText(/delete-ui-test/);
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('autosave_status',{slotId:'slot'}));
 fireEvent.click(screen.getByRole('button',{name:'自作サンプル（オフライン）'}));
 const map=screen.getByRole('img',{name:'駅配置用サンプル地図'});
 map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 fireEvent.click(screen.getByRole('tab',{name:'線路'}));
 await settledMapClick(map,{clientX:80,clientY:200,detail:1});
 await settledMapClick(map,{clientX:180,clientY:200,detail:1});
 await settledMapClick(map,{clientX:180,clientY:200,detail:1});
 const shapes=()=>Array.from(map.querySelectorAll('.draft-path')).map(p=>p.getAttribute('d'));
 const planned=shapes();expect(planned.length).toBeGreaterThan(0);
 fireEvent.click(screen.getByRole('tab',{name:'系統'}));
 fireEvent.click(screen.getByRole('button',{name:'系統編集: 普通'}));
 fireEvent.click(screen.getByRole('button',{name:'系統を削除'}));
 await mutate(()=>fireEvent.click(screen.getByRole('button',{name:'削除する'})),'delete_service');
 await waitFor(()=>expect(screen.queryByRole('button',{name:'系統編集: 普通'})).toBeNull());
 expect(shapes()).toEqual(planned);
 for(let i=0;i<2;i++){
  await mutate(()=>fireEvent.keyDown(document.body,{key:'z',ctrlKey:true}),'undo_world_edit');
  await screen.findByRole('button',{name:'系統編集: 普通'});expect(shapes()).toEqual(planned);
  await mutate(()=>fireEvent.keyDown(document.body,{key:'z',ctrlKey:true,shiftKey:true}),'redo_world_edit');
  await waitFor(()=>expect(screen.queryByRole('button',{name:'系統編集: 普通'})).toBeNull());expect(shapes()).toEqual(planned);
 }
 expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==='delete_service')).toHaveLength(1);
 expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==='undo_world_edit')).toHaveLength(2);
 expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==='redo_world_edit')).toHaveLength(2);
});
