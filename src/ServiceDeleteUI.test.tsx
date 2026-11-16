// @vitest-environment jsdom
import React from 'react';
import {act,cleanup,fireEvent,render,screen,waitFor,within} from '@testing-library/react';
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
test('service deletion Undo/Redo preserves newer unsaved construction plan',async()=>{
 const before={...structuredClone(base),construction_plans:emptyConstructionPlans()};
 const after={...before,services:[],service_paths:[],service_route_bindings:[],service_appearances:[]};
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'delete-ui-test'};
  if(command==='world_snapshot'||command==='undo_alignment_deletion')return structuredClone(before);
  if(command==='delete_service')return structuredClone(after);
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
 fireEvent.click(screen.getByRole('button',{name:'削除する'}));
 await waitFor(()=>expect(screen.queryByRole('button',{name:'系統編集: 普通'})).toBeNull());
 expect(shapes()).toEqual(planned);
 for(let i=0;i<2;i++){
  fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
  await screen.findByRole('button',{name:'系統編集: 普通'});expect(shapes()).toEqual(planned);
  fireEvent.keyDown(document.body,{key:'z',ctrlKey:true,shiftKey:true});
  await waitFor(()=>expect(screen.queryByRole('button',{name:'系統編集: 普通'})).toBeNull());expect(shapes()).toEqual(planned);
 }
});
