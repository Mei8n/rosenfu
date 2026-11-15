// @vitest-environment jsdom
import React from 'react';
import {cleanup,fireEvent,render,screen,waitFor,within} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
const provider=vi.hoisted(()=>({load:vi.fn()}));
vi.mock('./roadProvider',()=>({createRoadProvider:()=>({name:'acceptance fixture',load:provider.load})}));
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:()=>true}));
import {invoke} from '@tauri-apps/api/core';
import {App} from './App';
const empty={view_state:{latitude:35.67504,longitude:139.75,zoom:16},next_id:'3',players:[{id:'1',name:'Player'}],companies:[{id:'2',owner:'1',name:'Railway'}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
afterEach(()=>{cleanup();vi.clearAllMocks();vi.unstubAllGlobals();localStorage.clear();});
async function start(){
 vi.stubGlobal('PointerEvent',MouseEvent);
 vi.stubGlobal('fetch',vi.fn().mockRejectedValue(Error('network intentionally unavailable')));
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'road-water-acceptance'};
  if(command==='world_snapshot'||command==='build_plan'||command==='build_alignment')return empty;
  if(command==='list_save_slots')return {active_slot_id:'test',slots:[]};
  if(command==='autosave_status')return {available:false};
  throw Error('unexpected '+command);
 });
 render(<App/>);fireEvent.click(screen.getByRole('button',{name:'自作サンプル（オフライン）'}));
 await screen.findByText(/road-water-acceptance/);
 const map=screen.getByRole('img',{name:'駅配置用サンプル地図'});
 map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 return map;
}
test('unavailable road reference does not disable ordinary construction or claim water approval',async()=>{
 provider.load.mockRejectedValue(Error('outside road cells'));
 const map=await start();fireEvent.click(screen.getByRole('button',{name:'道路に合わせる'}));
 await waitFor(()=>expect(screen.getByRole('status',{name:'道路データ状態'}).textContent).toContain('outside road cells'));
 expect(screen.getByRole('status',{name:'道路データ状態'}).textContent).toContain('水域建設判定は未実装');
 expect(screen.getByRole('status',{name:'道路データ状態'}).textContent).toContain('建設時の道路参照から踏切を判定');
 expect(screen.getByRole('status',{name:'道路データ状態'}).textContent).not.toContain('踏切自動作成・水域建設判定は未実装');
 fireEvent.click(screen.getByRole('button',{name:'道路に合わせる'}));
 fireEvent.click(map,{clientX:180,clientY:130});fireEvent.click(map,{clientX:320,clientY:130});
 fireEvent.keyDown(document.body,{key:'Escape'});
 const button=screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement;
 expect(button.disabled).toBe(false);fireEvent.click(button);
 await waitFor(()=>expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='build_plan'||command==='build_alignment')).toBe(true));
 expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='preview_environment'||command==='build_plan_with_environment')).toBe(false);
 expect(document.body.textContent).not.toContain('水域取得済み');
});
test('road acquisition failure reason is in the existing near-track window, not only the side panel',async()=>{
 provider.load.mockRejectedValue(Error('outside road cells'));
 const map=await start();fireEvent.click(screen.getByRole('button',{name:'道路に合わせる'}));
 await waitFor(()=>expect(screen.getByRole('status',{name:'道路データ状態'}).textContent).toContain('outside road cells'));
 fireEvent.pointerMove(map,{clientX:250,clientY:150});
 const tooltip=document.querySelector<HTMLElement>('.drawing-tooltip');
 expect(tooltip).not.toBeNull();expect(tooltip?.textContent).toContain('道路参考データ未取得');
 expect(tooltip?.textContent).toContain('outside road cells');
 expect(document.querySelectorAll('.construction-tooltip')).toHaveLength(1);
 expect(within(tooltip!).queryByRole('button')).toBeNull();
});
test('automatic road source has only visibility controls and retains attribution when hidden',async()=>{
 const {parseRoadResponse}=await import('./automaticRoadData');
 const bounds={west:139.74,east:139.76,south:35.67,north:35.69};
 const snapshot=await parseRoadResponse({osm3s:{timestamp_osm_base:'2026-11-14T00:00:00Z'},elements:[{type:'way',id:'1',nodes:['1','2'],geometry:[{lat:35.675,lon:139.75},{lat:35.676,lon:139.751}],tags:{highway:'service'}}]},bounds,0);
 provider.load.mockResolvedValue(snapshot);
 await start();fireEvent.click(screen.getByRole('button',{name:'道路に合わせる'}));
 await waitFor(()=>expect(screen.getByRole('status',{name:'道路データ状態'}).textContent).toContain('道路参考データ取得済み'));
 expect(screen.queryByRole('button',{name:'参考線を外す'})).toBeNull();
 fireEvent.click(screen.getByRole('checkbox',{name:'道路参考線を表示'}));
 expect(screen.getByRole('link',{name:'© OpenStreetMap contributors'})).toBeTruthy();
});
