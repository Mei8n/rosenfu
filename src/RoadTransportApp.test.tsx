// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {App} from './App';
vi.mock('@tauri-apps/api/core',()=>({isTauri:()=>true,invoke:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
const world={next_id:'10',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[],routes:[],services:[]};
function mockEngine(available:boolean){
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'road-app-test',road_transport_enabled:available};
  if(command==='list_save_slots')return {active_slot_id:null,slots:[]};
  if(command==='ofm_world_token')return 'current-world-token';
  if(command==='enable_road_qa_fixture')return {...world,road_transport:{nodes:[{id:'a',point:{latitude:35,longitude:139}},{id:'b',point:{latitude:35,longitude:139.01}}],edges:[{id:'e',from:'a',to:'b',source:'synthetic-qa-not-real-roads'}],bus_stops:[],bus_routes:[],tram_sections:[]}};
  return world;
 });
}
test('older native engines never expose the road QA controls',async()=>{
 mockEngine(false);render(<App/>);await screen.findByText(/road-app-test/);
 expect(screen.queryByRole('tab',{name:'道路交通（検証）'})).toBeNull();
});
test('the road QA tab requires opt-in, uses a native world token and marks the world dirty',async()=>{
 mockEngine(true);render(<App/>);await screen.findByText(/road-app-test/);
 const select=screen.getByRole('tab',{name:'選択'});fireEvent.keyDown(select,{key:'End'});
 expect(screen.getByRole('tab',{name:'道路交通（検証）'}).getAttribute('aria-selected')).toBe('true');
 const add=screen.getByRole('button',{name:'検証用道路を追加'});
 expect((add as HTMLButtonElement).disabled).toBe(true);fireEvent.click(add);
 expect(invoke).not.toHaveBeenCalledWith('ofm_world_token');
 fireEvent.click(screen.getByLabelText('実在道路ではない検証用データを使用する'));fireEvent.click(add);
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('enable_road_qa_fixture',{confirmedSynthetic:true,expectedWorldToken:'current-world-token'}));
 await screen.findByText('未保存の変更あり');
 expect(screen.getByText(/検証専用の架空道路/)).toBeTruthy();
});
const roadPoints=[{id:'qa:0',point:{latitude:35,longitude:135}},{id:'qa:1',point:{latitude:35,longitude:135.005}},{id:'qa:2',point:{latitude:35.005,longitude:135.005}}];
const qaWorld={...world,road_transport:{nodes:roadPoints,edges:[{id:'edge',from:'qa:0',to:'qa:1',source:'synthetic-qa-not-real-roads',bus_access:'allowed',tram_access:'allowed',geometry:roadPoints.slice(0,2).map(node=>node.point)},{id:'edge2',from:'qa:1',to:'qa:2',source:'synthetic-qa-not-real-roads',bus_access:'allowed',tram_access:'allowed',geometry:roadPoints.slice(1).map(node=>node.point)}],bus_stops:[],bus_routes:[],tram_sections:[]}};
async function showRoadMap(tram:boolean,bus=false){
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'road-map-test',road_transport_enabled:true,road_tram_enabled:tram,bus_operation_enabled:bus,operation_enabled:bus};
  if(command==='list_save_slots')return {active_slot_id:null,slots:[]};
  if(command==='ofm_world_token')return 'current-world-token';
  if(command==='operation_snapshot')return {now_ms:0,paused:true,speed:1,invalidated:false,catching_up:false,timetable_edit_token:'fixture-token',services:[],trains:[],bus_services:[]};
  return bus?{...qaWorld,road_transport:{...qaWorld.road_transport,bus_routes:[{id:'test-route',name:'自社の検証バス',owner:'2',stops:[],edges:[]},{id:'other-route',name:'他社の検証バス',owner:'3',stops:[],edges:[]}]}}:qaWorld;
 });
 render(<App/>);await screen.findByText(/road-map-test/);fireEvent.click(screen.getByRole('tab',{name:'道路交通（検証）'}));
}
const mainPick=()=>fireEvent.click(screen.getByRole('button',{name:'地図で起点・終点を選ぶ'}));
const roadNode=(id:number)=>screen.getByRole('button',{name:`地図上の道路点 qa:${id}`});
test('main map uses real QA geometry and requires two different points; no implicit mutation',async()=>{
 await showRoadMap(false);expect(screen.queryByRole('button',{name:'軌道を着工'})).toBeNull();
 mainPick();const first=roadNode(0);
 expect(Number(first.getAttribute('cx'))).toBeGreaterThan(0);
 expect(Number(first.getAttribute('cx'))).toBeLessThan(1000);
 fireEvent.click(first);fireEvent.click(first);
 expect(screen.getByText('検証用道路の終点を選択')).toBeTruthy();
 fireEvent.keyDown(roadNode(2),{key:'Enter'});
 expect(screen.queryByText('検証用道路の終点を選択')).toBeNull();
 expect((screen.getByLabelText(/^起点$/) as HTMLSelectElement).value).toBe('qa:0');
 expect((screen.getByLabelText(/^終点$/) as HTMLSelectElement).value).toBe('qa:2');
 expect(invoke).not.toHaveBeenCalledWith('ofm_world_token');
});
test('Escape and switching tabs discard a half-picked pair; tram gate controls its own action',async()=>{
 await showRoadMap(true);expect((screen.getByRole('button',{name:'軌道を着工'}) as HTMLButtonElement).disabled).toBe(true);
 mainPick();fireEvent.click(roadNode(1));fireEvent.keyDown(window,{key:'Escape'});
 expect(screen.queryByRole('button',{name:'地図上の道路点 qa:1'})).toBeNull();
 expect((screen.getByLabelText(/^起点$/) as HTMLSelectElement).value).toBe('');
 mainPick();fireEvent.click(roadNode(1));fireEvent.click(screen.getByRole('tab',{name:'選択'}));
 expect(screen.queryByRole('button',{name:'地図上の道路点 qa:1'})).toBeNull();
 fireEvent.click(screen.getByRole('tab',{name:'道路交通（検証）'}));mainPick();
 expect(screen.getByText('検証用道路の起点を選択')).toBeTruthy();
 fireEvent.click(roadNode(0));fireEvent.click(roadNode(2));
 fireEvent.click(screen.getByRole('button',{name:'軌道を着工'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('edit_road_tram',{expectedWorldToken:'current-world-token',edit:{kind:'build',from:'qa:0',to:'qa:2',shared:true,name:'軌道'}}));
});

test('only owned bus routes use the supported native operation panel',async()=>{
 await showRoadMap(false,true);
 const route=await screen.findByRole('combobox',{name:'運行するバス系統'});
 expect((route as HTMLSelectElement).value).toBe('test-route');
 expect(screen.queryByRole('option',{name:'他社の検証バス'})).toBeNull();
 expect(screen.getByRole('button',{name:'運行を設定'})).toBeTruthy();
 expect(invoke).not.toHaveBeenCalledWith('configure_bus_operation',expect.anything());
});

test('fictional boarding snaps only the explicitly chosen directed road on the main map',async()=>{
 const a={latitude:35,longitude:139},b={latitude:35,longitude:139.006};
 const edge={id:'forward',from:'a',to:'b',source:'native-pilot:verified-region',revision:'2',geometry:[a,b],bus_access:'allowed',tram_access:'unknown',level:'ground'};
 const roadWorld={...world,road_transport:{nodes:[{id:'a',point:a},{id:'b',point:b}],edges:[edge,{...edge,id:'reverse',from:'b',to:'a',geometry:[b,a]}],bus_stops:[],bus_routes:[],tram_sections:[]}};
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'player-road-test',road_transport_enabled:true};
  if(command==='list_save_slots')return {active_slot_id:null,slots:[]};
  if(command==='ofm_world_token')return 'current-world-token';
  if(command==='list_native_road_regions')return [{id:'verified-region',region:'地域',origin:'Fixture',revision:'2',attribution:'Fixture',bus_stop_positions:[],allows_player_bus_stops:true}];
  return roadWorld;
 });
 render(<App/>);await screen.findByText(/player-road-test/);
 fireEvent.click(screen.getByRole('tab',{name:'道路交通（検証）'}));
 const first=await screen.findByLabelText('乗車の道路');
 const second=screen.getByLabelText('降車の道路');
 fireEvent.change(first,{target:{value:'reverse'}});
 fireEvent.change(second,{target:{value:'reverse'}});
 expect(screen.queryByRole('button',{name:'地図で起点・終点を選ぶ'})).toBeNull();
 const view=screen.getByRole('img',{name:/^駅配置用/});
 view.getBoundingClientRect=()=>new DOMRect(0,0,1000,600);
 function selectAt(frac:number,dy=0){
  const line=screen.getByLabelText('停留所を置く道路');
  const [x0,y0,x1,y1]=[...(line.getAttribute('d')??'').matchAll(/-?\d+(?:\.\d+)?/g)].map(m=>Number(m[0]));
  expect(x0).toBeGreaterThan(x1); // same shape as forward, but not its direction
  fireEvent.click(view,{clientX:x0+(x1-x0)*frac,clientY:y0+(y1-y0)*frac+dy});
 }
 const openBoarding=()=>fireEvent.click(screen.getByRole('button',{name:'地図で乗車位置を選ぶ'}));
 openBoarding();
 selectAt(0.25,30);
 expect(screen.getByText('強調された道路上で乗車位置を選択')).toBeTruthy();
 fireEvent.keyDown(window,{key:'Escape'});
 expect(screen.queryByLabelText('停留所を置く道路')).toBeNull();
 expect(screen.getByRole<HTMLInputElement>('slider',{name:'乗車の位置'}).value).toBe('50');
 openBoarding();fireEvent.click(screen.getByRole('tab',{name:'選択'}));
 expect(screen.queryByLabelText('停留所を置く道路')).toBeNull();
 fireEvent.click(screen.getByRole('tab',{name:'道路交通（検証）'}));
 openBoarding();selectAt(0.25);
 expect(screen.getByRole<HTMLInputElement>('slider',{name:'乗車の位置'}).value).toBe('25');
 fireEvent.click(screen.getByRole('button',{name:'地図で降車位置を選ぶ'}));selectAt(0.75);
 expect(screen.getByRole<HTMLInputElement>('slider',{name:'降車の位置'}).value).toBe('75');
 expect(invoke).not.toHaveBeenCalledWith('ofm_world_token');
 fireEvent.click(screen.getByRole('button',{name:'停留所とバス系統を作成'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('edit_native_pilot_bus',{
  expectedWorldToken:'current-world-token',
  edit:{route_id:expect.stringMatching(/^bus:/),name:'バス系統',expected_route:null,
   from:{edge_id:'reverse',position:0.25,location_kind:'player_created'},
   to:{edge_id:'reverse',position:0.75,location_kind:'player_created'}},
 }));
});
