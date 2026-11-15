// @vitest-environment jsdom
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
const provider=vi.hoisted(()=>({load:vi.fn()}));
vi.mock('./roadProvider',()=>({createRoadProvider:()=>({name:'road test fixture',load:provider.load})}));
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:()=>true}));
import {invoke} from '@tauri-apps/api/core';
import {App} from './App';
import {unproject} from './map';
import {parseRoadResponse} from './automaticRoadData';
const empty={view_state:{latitude:35.67504,longitude:139.75,zoom:16},next_id:'3',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'Railway'}],stations:[],routes:[],services:[],alignments:[],station_placements:[]};
afterEach(()=>{cleanup();vi.useRealTimers();vi.resetAllMocks();vi.unstubAllGlobals();localStorage.clear();});
async function start(snapshot:unknown=empty,draw=true){
 vi.stubGlobal('PointerEvent',MouseEvent);vi.stubGlobal('fetch',vi.fn().mockRejectedValue(Error('offline')));
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'crossing-test'};
  if(command==='world_snapshot')return snapshot;
  if(command==='build_alignment')return empty;
  if(command==='list_save_slots')return {active_slot_id:'test',slots:[]};
  if(command==='autosave_status')return {available:false};
  if(command==='build_plan_with_roads'){
   const request=args as {roads:unknown[];alignments:{points:unknown[]}[]};
   return {...empty,road_snapshots:request.roads,alignments:[{id:'3',owner:'2',name:'Rail',track_type:'normal',points:request.alignments[0].points}]};
  }
  if(command==='road_assessments')return [['3',{roads_ready:true,unknown_road_height:false,crossings:[{road_id:'1',point:{latitude:35.67504,longitude:139.75},position:.5}]}]];
  throw Error('unexpected '+command);
 });
 render(<App/>);fireEvent.click(screen.getByRole('button',{name:'自作サンプル（オフライン）'}));await screen.findByText(/crossing-test/);
 expect(screen.getByRole('button',{name:'道路に合わせる'}).getAttribute('aria-pressed')).toBe('false');
 const map=screen.getByRole('img',{name:'駅配置用サンプル地図'});map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 if(draw){fireEvent.click(map,{clientX:180,clientY:130});fireEvent.click(map,{clientX:320,clientY:130});fireEvent.keyDown(document.body,{key:'Escape'});}return map;
}
test('road following OFF still loads construction roads and atomically attaches immutable facts',async()=>{
 provider.load.mockImplementation(async bounds=>parseRoadResponse({osm3s:{timestamp_osm_base:'2026-11-14T21:20:41Z'},elements:[{type:'way',id:'1',nodes:['1','2'],geometry:[{lat:35.67,lon:139.75},{lat:35.68,lon:139.75}],tags:{highway:'service'}}]},bounds,0));
 const map=await start();
 await waitFor(()=>expect(screen.getByRole('status',{name:'踏切・道路確認状態'}).textContent).toContain('出典の範囲保証は未検証'));
 fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('build_plan_with_roads',expect.objectContaining({roads:[expect.objectContaining({coverage_verified:false,roads:expect.any(Array)})]})));
 await waitFor(()=>expect(map.querySelectorAll('[data-crossing-alignment]')).toHaveLength(1));
});
test('missing road data remains advisory and ordinary construction still succeeds',async()=>{
 provider.load.mockRejectedValue(Error('outside Tokyo cells'));
 await start();await waitFor(()=>expect(screen.getByRole('status',{name:'踏切・道路確認状態'}).textContent).toContain('outside Tokyo cells'));
 const button=screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement;expect(button.disabled).toBe(false);fireEvent.click(button);
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('build_alignment',expect.any(Object)));
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='build_plan_with_environment')).toBe(false);
});


test('junction Undo and Redo use exactly the same road snapshot payload',async()=>{
 provider.load.mockImplementation(async bounds=>parseRoadResponse({osm3s:{timestamp_osm_base:'2026-11-14T21:20:41Z'},elements:[]},bounds,0));
 const source={id:'3',owner:'2',name:'Main',track_type:'normal',points:[unproject(100,300,empty.view_state),unproject(900,300,empty.view_state)],anchor_indices:[0,1]};
 const before={...empty,next_id:'4',alignments:[source]};
 const map=await start(before,false);
 fireEvent.click(map,{clientX:200,clientY:150});fireEvent.click(map,{clientX:300,clientY:170});fireEvent.click(map,{clientX:300,clientY:170});
 await waitFor(()=>expect(screen.getByRole('status',{name:'踏切・道路確認状態'}).textContent).toContain('出典の範囲保証は未検証'));
 const original=vi.mocked(invoke).getMockImplementation()!;
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command==='undo_alignment_deletion')return before;
  if(command==='build_plan_with_roads')return {...before,next_id:'5',road_snapshots:(args as {roads:unknown[]}).roads,alignments:[source,{...source,id:'4'}]};
  return original(command,args);
 });
 await act(async()=>fireEvent.click(screen.getByRole('button',{name:'計画を着工'})));
 expect(map.querySelectorAll('[data-alignment-id]')).toHaveLength(2);
 await act(async()=>fireEvent.keyDown(document.body,{key:'z',ctrlKey:true}));
 expect(invoke).toHaveBeenCalledWith('undo_alignment_deletion');expect(map.querySelectorAll('[data-alignment-id]')).toHaveLength(1);
 await act(async()=>fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true}));
 const builds=vi.mocked(invoke).mock.calls.filter(([command])=>command==='build_plan_with_roads');
 expect(builds).toHaveLength(2);expect(builds[1][1]).toEqual(builds[0][1]);
});


test('early build waits for the shared road download instead of silently omitting crossings',async()=>{
 let finish!:(value:Awaited<ReturnType<typeof parseRoadResponse>>)=>void;
 provider.load.mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
 await start();fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));
 expect(screen.getByText(/道路を確認中（最大25秒）/)).toBeTruthy();
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='build_alignment'||c==='build_plan_with_roads')).toBe(false);
 await waitFor(()=>expect(provider.load).toHaveBeenCalledTimes(1));
 const bounds=provider.load.mock.calls[0][0];
 const snapshot=await parseRoadResponse({osm3s:{timestamp_osm_base:'2026-11-14T21:20:41Z'},elements:[]},bounds,0);
 await act(async()=>finish(snapshot));
 expect(provider.load).toHaveBeenCalledTimes(1);
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('build_plan_with_roads',expect.objectContaining({roads:expect.any(Array)})));
});

test('cancel during road verification never builds, even after the shared download completes',async()=>{
 let finish!:(value:Awaited<ReturnType<typeof parseRoadResponse>>)=>void;
 provider.load.mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
 const map=await start();fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));
 fireEvent.click(await screen.findByRole('button',{name:'着工を取り消す'}));
 await screen.findByText('着工を取り消しました。下書きは保持しています。');
 const snapshot=await parseRoadResponse({osm3s:{timestamp_osm_base:'2026-11-14T21:20:41Z'},elements:[]},provider.load.mock.calls[0][0],0);
 await act(async()=>finish(snapshot));
 expect(map.querySelectorAll('.draft-anchor').length).toBeGreaterThan(0);
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='build_alignment'||c==='build_plan_with_roads')).toBe(false);
});

test('deadline fallback builds normally and retains an explicit unverified-crossing notice',async()=>{
 let finish!:(value:Awaited<ReturnType<typeof parseRoadResponse>>)=>void;
 provider.load.mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
 await start();vi.useFakeTimers();
 fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));
 await act(()=>vi.advanceTimersByTimeAsync(25000));
 expect(invoke).toHaveBeenCalledWith('build_alignment',expect.any(Object));
 expect(screen.getByText(/踏切未確認：Error: 道路確認が25秒/)).toBeTruthy();
 const snapshot=await parseRoadResponse({osm3s:{timestamp_osm_base:'2026-11-14T21:20:41Z'},elements:[]},provider.load.mock.calls[0][0],0);
 await act(async()=>finish(snapshot));
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='build_plan_with_roads')).toBe(false);
});
