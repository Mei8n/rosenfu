// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
vi.mock('./useAutomaticRoad',()=>({useAutomaticRoad:()=>({status:'unavailable',message:'道路未取得',retry:()=>{},ensureCurrent:async()=>({status:'stale'})})}));
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:()=>true}));
import {invoke} from '@tauri-apps/api/core';
import {App} from './App';
const empty={view_state:{latitude:35.67504,longitude:139.75,zoom:16},next_id:'3',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'Railway'}],stations:[],routes:[],services:[],alignments:[],station_placements:[]};
afterEach(()=>{cleanup();vi.resetAllMocks();localStorage.clear();});
test('a stale road verification aborts construction rather than falling back with the old plan',async()=>{
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'stale-test'};
  if(command==='world_snapshot')return empty;
  if(command==='list_save_slots')return {active_slot_id:'test',slots:[]};
  if(command==='autosave_status')return {available:false};
  throw Error('unexpected '+command);
 });
 render(<App/>);fireEvent.click(screen.getByRole('button',{name:'自作サンプル（オフライン）'}));await screen.findByText(/stale-test/);
 const map=screen.getByRole('img',{name:'駅配置用サンプル地図'});map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 fireEvent.click(map,{clientX:180,clientY:130});fireEvent.click(map,{clientX:320,clientY:130});fireEvent.keyDown(document.body,{key:'Escape'});
 fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));
 await screen.findByText(/計画またはワールドが変更されたため着工を中止/);
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='build_alignment'||c==='build_plan'||c==='build_plan_with_roads')).toBe(false);
 expect(map.querySelectorAll('.draft-anchor').length).toBeGreaterThan(0);
});
