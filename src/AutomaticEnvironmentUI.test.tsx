// @vitest-environment jsdom
import React from 'react';
import {it,expect,vi,afterEach} from 'vitest';
import {render,screen,fireEvent,waitFor,cleanup,act} from '@testing-library/react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
const state=vi.hoisted(()=>({ready:false,buildable:false}));
const data={revision:'fixture',roads:[],road_coverage:[],water:[],water_coverage:[]};
vi.mock('./useAutomaticEnvironment',()=>({useAutomaticEnvironment:()=>({road:{message:state.ready?'道路取得済み':'道路データ未取得'},water:{message:'水域fixture'},data:state.ready?{revision:'fixture',roads:[],road_coverage:[],water:[],water_coverage:[]}:null,retry:()=>{}})}));
vi.mock('./useEnvironmentPreview',()=>({useEnvironmentPreview:()=>state.ready?{preview:{buildable:state.buildable,crossings:[{road_id:'10',point:{longitude:139.75,latitude:35.675}}],assessments:[{crossings:[],roads_ready:true,water_ready:true,unknown_road_height:false,ground_water:!state.buildable,longest_water_m:0,water_limit_exceeded:false}]}}:{}}));
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();localStorage.clear();});
it('blocks unknown and forbidden water, then submits checked snapshot and renders derived crossing',async()=>{
 state.ready=false;state.buildable=false;vi.mocked(isTauri).mockReturnValue(true);
 const world={next_id:'3',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[],routes:[],services:[],alignments:[]};
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'environment-test'};
  if(command==='world_snapshot')return world;
  if(command==='list_save_slots')return {active_slot_id:'slot',slots:[{id:'slot',name:'test',loadable:true}]};
  if(command==='autosave_status')return {available:false};
  if(command==='build_plan_with_environment')return {...world,environment:data};
  if(command==='automatic_crossings')return [{road_id:'10',point:{longitude:139.75,latitude:35.675}}];
  throw Error('unexpected '+command+JSON.stringify(args));
 });
 const ui=render(<App/>);fireEvent.click(screen.getByRole('button',{name:'自作サンプル（オフライン）'}));await screen.findByText(/environment-test/);
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('autosave_status',{slotId:'slot'}));await act(async()=>{});
 const map=screen.getByRole('img',{name:'駅配置用サンプル地図'});map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 fireEvent.click(map,{clientX:180,clientY:130});fireEvent.click(map,{clientX:230,clientY:130});
 const build=()=>screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement;
 expect(build().disabled).toBe(true);expect(screen.getByText(/道路データ未取得/)).toBeTruthy();
 state.ready=true;ui.rerender(<App/>);expect(build().disabled).toBe(true);expect(screen.getByText(/地上線は水面/)).toBeTruthy();
 state.buildable=true;ui.rerender(<App/>);expect(build().disabled).toBe(false);fireEvent.click(build());
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('build_plan_with_environment',expect.objectContaining({environment:data})));
 await waitFor(()=>expect(map.querySelectorAll('[data-automatic-crossing]')).toHaveLength(1));
});
