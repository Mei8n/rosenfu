// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {App} from './App';
import {project} from './map';

const {read,observe}=vi.hoisted(()=>({read:vi.fn(),observe:vi.fn()}));
vi.mock('@tauri-apps/api/core',()=>({isTauri:()=>true,invoke:vi.fn()}));
const a={latitude:35.67504,longitude:139.748},b={latitude:35.67504,longitude:139.752};
const bend={latitude:35.67532,longitude:139.75};
const bridgeOnly={latitude:35.67424,longitude:139.7535};
const waypoint={latitude:35.67482,longitude:139.75},waypointMoved={latitude:35.67452,longitude:139.7508};
const raw='2'.repeat(64),key='https://tiles.openfreemap.org/planet/r1/14/14552/6452.pbf';
const tile={bundle:{sourceSha256:raw,source:{source:key}},tileLines:[{
 id:'ofm:r1:14:14552:6452:0:0',z:14,x:14552,y:6452,revision:'r1',class:'minor',coordinates:[[a.longitude,a.latitude],[b.longitude,b.latitude]]
}]};
const left={latitude:a.latitude,longitude:139.744};
const leftRaw='3'.repeat(64),leftKey='https://tiles.openfreemap.org/planet/r1/14/14551/6452.pbf';
const adjacent={bundle:{sourceSha256:leftRaw,source:{source:leftKey}},tileLines:[{
 id:'ofm:r1:14:14551:6452:4:2',z:14,x:14551,y:6452,revision:'r1',class:'minor',coordinates:[[left.longitude,left.latitude],[139.745,left.latitude]]
}]};
let snapshots=[tile];
vi.mock('./useAutomaticRoad',()=>({useAutomaticRoad:(_bounds:unknown,enabled:boolean)=>enabled?{
 status:'ready',snapshot:{tileSnapshots:snapshots},readTileEvidenceBatch:read,observeRouteTiles:observe,
}:{status:'unavailable',readTileEvidenceBatch:read}}));
class TestPointerEvent extends MouseEvent {
 readonly pointerId:number;readonly isPrimary:boolean;
 constructor(type:string,init?:PointerEventInit){
  super(type,init);this.pointerId=init?.pointerId??1;this.isPrimary=init?.isPrimary??true;
 }
}
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();vi.unstubAllGlobals();});
const world={next_id:'10',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[],routes:[],services:[]};
async function setup(config:{preview:()=>Promise<unknown>;ordinary?:boolean;rectangle?:boolean;multiple?:boolean;autoStart?:boolean;roundTrip?:boolean;operation?:boolean;waypoints?:boolean;savedProof?:boolean;interiorStops?:boolean;autoStops?:boolean;closed?:boolean;closedSaved?:boolean;observedGrade?:boolean;retainedGrade?:boolean;retainedOneway?:boolean}){
 const authored=config.waypoints?{...tile,tileLines:[{...tile.tileLines[0],coordinates:[
  ...tile.tileLines[0].coordinates,[waypoint.longitude,waypoint.latitude],[waypointMoved.longitude,waypointMoved.latitude],
 ]}]}:tile;
 const candidate=config.observedGrade?{...authored,tileLines:[...authored.tileLines,{
  ...tile.tileLines[0],id:'ofm:r1:14:14552:6452:2:0',brunnel:'bridge',
  coordinates:[[bridgeOnly.longitude,bridgeOnly.latitude],[bridgeOnly.longitude+.0006,bridgeOnly.latitude-.00025]],
 }]}:authored;
 snapshots=config.rectangle?[adjacent,candidate]:[candidate];
 const bind=(x:number,sha:string,url:string)=>({key:url,url,x,y:6452,z:14,revision:'r1',rawSha256:sha,decoderRule:'ofm_transportation_z14_v1',license:'ODbL-1.0',attribution:'© OpenMapTiles © OpenStreetMap contributors'});
 const bindings=config.rectangle?[bind(14551,leftRaw,leftKey),bind(14552,raw,key)]:[bind(14552,raw,key)];
 observe.mockResolvedValue(bindings);
 read.mockResolvedValue({status:'ready',evidence:{
  rawTiles:bindings.map((entry,i)=>({sha256:entry.rawSha256,bytes:Uint8Array.of(21+i)})),bindings,
 }});
 const first={id:'owned-a',name:'既存 A',owner:'2',edges:['native-edge'],stops:[]};
 const second={...first,id:'owned-b',name:'既存 B'};
 const foreign={...first,id:'foreign',owner:'3',name:'他社 C'};
 const nonordinary={...first,id:'pilot',name:'対象外 D'};
 const baseline=config.multiple?{
  ...world,ofm_gameplay_bus_routes:{[first.id]:config.savedProof?{
   rule:'ofm_ground_bus_z14_v1',tileKey:key,tileRawSha256:raw,respectOneway:!!config.retainedOneway,
   start:{feature:0,line:0,vertex:0},end:{feature:0,line:0,vertex:config.closedSaved?0:1},
   ...(config.retainedGrade?{gradePolicy:'observedConnectors'}:{}),
   ...(config.closedSaved?{pattern:'closedOneWay',via:[{id:'closed-via',tileKey:key,vertex:{feature:0,line:0,vertex:1}}]}:{}),
  }:{},[second.id]:{},[foreign.id]:{}},
  road_transport:{nodes:[],edges:[{id:'native-edge',geometry:[a,bend,b]}],bus_stops:[],tram_sections:[],
   bus_routes:[first,second,foreign,nonordinary]},
 }:undefined;
 vi.mocked(invoke).mockImplementation(async (command,args)=>{
  if(command==='engine_info')return {version:'ordinary-test',road_transport_enabled:true,ordinary_ofm_bus_enabled:config.ordinary!==false,ordinary_ofm_bus_rectangle_enabled:config.rectangle===true,ordinary_ofm_bus_multiple_enabled:config.multiple===true,ordinary_ofm_bus_round_trip_enabled:config.roundTrip===true,ordinary_ofm_bus_waypoints_enabled:config.waypoints===true,ordinary_ofm_bus_interior_stops_enabled:config.interiorStops===true,ordinary_ofm_bus_auto_stops_enabled:config.autoStops===true,ordinary_ofm_bus_closed_loop_enabled:config.closed===true,ordinary_ofm_bus_observed_grade_enabled:config.observedGrade===true,operation_enabled:config.operation===true,bus_operation_enabled:config.operation===true};
  if(command==='ofm_world_token')return 'world-exact';
  if(command==='operation_snapshot')return {now_ms:0,paused:true,speed:1,invalidated:false,catching_up:false,
    timetable_edit_token:'fixture-operation-token',services:[],trains:[],bus_services:[]};
  if(command==='list_save_slots')return {active_slot_id:null,slots:[]};
  if(command==='preview_gameplay_ofm_bus')return config.preview();
  if(command==='put_gameplay_ofm_bus'){
   const req=args as {routeId:string;name:string;path:unknown};
   if(baseline)return {...baseline,ofm_gameplay_bus_routes:{...baseline.ofm_gameplay_bus_routes,[req.routeId]:req.path},
    road_transport:{...baseline.road_transport,bus_routes:[
     ...baseline.road_transport.bus_routes.filter(route=>route.id!==req.routeId),{...first,id:req.routeId,name:req.name},
    ]}};
   return {...world,ofm_gameplay_bus:req.path,road_transport:{nodes:[],edges:[{id:'native-edge',geometry:[a,bend,b]}],bus_stops:[],tram_sections:[],bus_routes:[{id:req.routeId,owner:'2',name:req.name,edges:['native-edge'],stops:[]}]}};
  }
  if(command==='remove_gameplay_ofm_bus'&&baseline){
   const routeId=(args as {routeId:string}).routeId;
   const remaining=Object.fromEntries(Object.entries(baseline.ofm_gameplay_bus_routes).filter(([id])=>id!==routeId));
   return {...baseline,ofm_gameplay_bus_routes:remaining,road_transport:{...baseline.road_transport,
    bus_routes:baseline.road_transport.bus_routes.filter(route=>route.id!==routeId),
   }};
  }
  return baseline??world;
 });
 render(<App/>);await screen.findByText(/ordinary-test/);
 if(config.ordinary===false)return;
 fireEvent.click(screen.getByRole('tab',{name:'バス'}));
 if(config.autoStart!==false)fireEvent.click(screen.getByRole('button',{name:'新しいバス系統'}));
 const map=screen.getByRole('img',{name:/^駅配置用/});map.getBoundingClientRect=()=>new DOMRect(0,0,1000,600);
 vi.stubGlobal('PointerEvent',TestPointerEvent);
 const pointerAt=(x:number,button=0)=>({clientX:x,clientY:300,pointerId:1,isPrimary:true,button});
 function press(x:number){
  const pointer=pointerAt(x);
  fireEvent.pointerDown(map,pointer);fireEvent.pointerUp(map,pointer);fireEvent.click(map,pointer);
 }
 function pressPoint(point:{latitude:number;longitude:number},detail=1,target:Element=map){
  const {x,y}=project(point.latitude,point.longitude,{latitude:a.latitude,longitude:139.75,zoom:16});
  const pointer={...pointerAt(x),clientY:y,detail};
  fireEvent.pointerDown(target,pointer);fireEvent.pointerUp(target,pointer);fireEvent.click(target,pointer);
 }
 return {from:()=>press(config.rectangle?220.38:406.8),to:()=>press(593.2),map,pointerAt,pressPoint};
}
test('a normal bus tool uses observed geometry and a native route before explicit save, without a debug import',async()=>{
 const map=(await setup({preview:async()=>({points:[a,bend,b],lengthM:400.5})}))!;
 map.from();await screen.findByText('道路上の終点を選択');
 expect(invoke).not.toHaveBeenCalledWith('ofm_world_token');
 map.to();await screen.findByText('道路経路 401 m・停留所2か所');
 expect(screen.getByLabelText('バス系統のプレビュー').getAttribute('d')?.match(/\bL/g)?.length).toBe(2);
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
 expect(invoke).not.toHaveBeenCalledWith('install_native_road_pilot',expect.anything());
 fireEvent.click(screen.getByRole('button',{name:'バス系統を確定'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.objectContaining({
  expectedWorldToken:'world-exact',path:expect.objectContaining({tileKey:key,respectOneway:false,start:{feature:0,line:0,vertex:0},end:{feature:0,line:0,vertex:1}}),
  rawEvidence:{sha256:raw,byteLength:1,base64:'FQ=='},
 })));
 await screen.findByRole('button',{name:'バス系統を引き直す'});
 expect(screen.getByLabelText('作成済みのバス経路')).toBeTruthy();
});
test('no path is not drawn as a straight fallback and can be cancelled',async()=>{
 const map=(await setup({preview:async()=>{throw '道路経路を確認できませんでした。';}}))!;
 map.from();map.to();await screen.findByText('道路経路を確認できませんでした。');
 expect(screen.queryByLabelText('バス系統のプレビュー')).toBeNull();
 expect(screen.queryByRole('button',{name:'バス系統を確定'})).toBeNull();
 fireEvent.keyDown(window,{key:'Escape'});
 expect(screen.getByRole('button',{name:'新しいバス系統'})).toBeTruthy();
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
});
test('does not offer the ordinary tool without a matching engine capability',async()=>{
 await setup({preview:async()=>null,ordinary:false});
 expect(screen.queryByRole('tab',{name:'バス'})).toBeNull();
 expect(screen.getByRole('tab',{name:'道路交通（検証）'})).toBeTruthy();
});
test('switching tools retires an outstanding native preview',async()=>{
 let resolve!:(v:unknown)=>void;
 const map=(await setup({preview:()=>new Promise(r=>{resolve=r;})}))!;
 map.from();map.to();
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('preview_gameplay_ofm_bus',expect.anything()));
 fireEvent.click(screen.getByRole('tab',{name:'選択'}));
 resolve({points:[a,b],lengthM:398});
 fireEvent.click(screen.getByRole('tab',{name:'バス'}));
 await screen.findByRole('button',{name:'新しいバス系統'});
 expect(screen.queryByLabelText('バス系統のプレビュー')).toBeNull();
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
});

test('ordinary bus crosses an adjacent boundary using the complete shared-cache native rectangle',async()=>{
 const map=(await setup({rectangle:true,preview:async()=>({points:[left,bend,b],lengthM:705})}))!;
 map.from();await screen.findByText('道路上の終点を選択');
 map.to();await screen.findByText('道路経路 705 m・停留所2か所');
 expect(observe).toHaveBeenCalledWith([[14551,6452],[14552,6452]],'r1',expect.any(AbortSignal));
 const previewArgs=vi.mocked(invoke).mock.calls.find(c=>c[0]==='preview_gameplay_ofm_bus')?.[1];
 expect(previewArgs).toMatchObject({
  expectedWorldToken:'world-exact',rawEvidence:null,tileBinding:null,path:{
   rule:'ofm_ground_bus_rectangle_z14_v2',tileKey:leftKey,tileRawSha256:leftRaw,
   endTileKey:key,additionalTiles:[{tileKey:key,tileRawSha256:raw}],
   start:{feature:4,line:2,vertex:0},end:{feature:0,line:0,vertex:1},respectOneway:false,
  },contextTiles:[{rawEvidence:{sha256:leftRaw},tileBinding:{key:leftKey}},{rawEvidence:{sha256:raw},tileBinding:{key}}],
 });
 fireEvent.click(screen.getByRole('button',{name:'バス系統を確定'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('put_gameplay_ofm_bus',previewArgs));
});

test('pan, middle-button clicks, and cancelled native pointer gestures never place a bus waypoint',async()=>{
 const {map,pointerAt}=(await setup({preview:async()=>({points:[a,b],lengthM:370})}))!;
 const first=pointerAt(406.8);
 fireEvent.pointerDown(map,first);fireEvent.pointerMove(map,pointerAt(456.8));fireEvent.pointerUp(map,pointerAt(456.8));
 fireEvent.click(map,pointerAt(456.8));
 expect(screen.getByText('道路上の起点を選択')).toBeTruthy();
 const current=pointerAt(456.8);
 fireEvent.pointerDown(map,{...current,button:1});fireEvent.pointerUp(map,{...current,button:1});fireEvent.click(map,current);
 expect(screen.getByText('道路上の起点を選択')).toBeTruthy();
 fireEvent.pointerDown(map,current);fireEvent.pointerCancel(map,current);fireEvent.click(map,current);
 expect(screen.getByText('道路上の起点を選択')).toBeTruthy();
 expect(screen.queryByLabelText('バスの乗車位置')).toBeNull();
 expect(invoke).not.toHaveBeenCalledWith('preview_gameplay_ofm_bus',expect.anything());
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
 fireEvent.pointerDown(map,current);fireEvent.pointerUp(map,current);fireEvent.click(map,current);
 await screen.findByText('道路上の終点を選択');
 expect(screen.getByLabelText('バスの乗車位置')).toBeTruthy();
 expect(invoke).not.toHaveBeenCalledWith('ofm_world_token');
});

test('selects owned ordinary routes, but opens a new map draft independently without overwriting them',async()=>{
 const map=(await setup({multiple:true,autoStart:false,preview:async()=>({points:[a,b],lengthM:310})}))!;
 const select=screen.getByRole<HTMLSelectElement>('combobox',{name:'表示するバス系統'});
 expect(select.value).toBe('owned-a');
 expect(Array.from(select.options).map(o=>o.text)).toEqual(['既存 A','既存 B']);
 fireEvent.change(select,{target:{value:'owned-b'}});
 fireEvent.click(screen.getByRole('button',{name:'バス系統を引き直す'}));
 expect(select.disabled).toBe(true);
 map.from();map.to();await screen.findByRole('button',{name:'バス系統を確定'});
 expect(invoke).toHaveBeenCalledWith('preview_gameplay_ofm_bus',expect.objectContaining({routeId:'owned-b',name:'既存 B'}));
 fireEvent.keyDown(window,{key:'Escape'});
 expect(select.disabled).toBe(false);
 fireEvent.click(screen.getByRole('button',{name:'新しいバス系統'}));
 map.from();map.to();await screen.findByRole('button',{name:'バス系統を確定'});
 const fresh=vi.mocked(invoke).mock.calls.filter(c=>c[0]==='preview_gameplay_ofm_bus').at(-1)?.[1] as {routeId:string};
 expect(fresh.routeId).toMatch(/^bus:/);
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
 fireEvent.click(screen.getByRole('button',{name:'バス系統を確定'}));
 await waitFor(()=>expect(select.value).toBe(fresh.routeId));
 expect(Array.from(select.options).map(o=>o.text)).toEqual(['既存 A','既存 B','バス系統']);
});

test('removes only the selected ordinary route after a separate confirmation',async()=>{
 await setup({multiple:true,autoStart:false,preview:async()=>null});
 const select=screen.getByRole<HTMLSelectElement>('combobox',{name:'表示するバス系統'});
 fireEvent.change(select,{target:{value:'owned-b'}});
 fireEvent.click(screen.getByRole('button',{name:'系統を削除'}));
 expect(invoke).not.toHaveBeenCalledWith('remove_gameplay_ofm_bus',expect.anything());
 fireEvent.click(screen.getByRole('button',{name:'削除を確定'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('remove_gameplay_ofm_bus',{expectedWorldToken:'world-exact',routeId:'owned-b'}));
 await screen.findByText('既存 A');
 expect(screen.queryByText('既存 B')).toBeNull();
});

test('a newly drawn open route requests and displays the native reverse path before committing two directions',async()=>{
 const reverseBend={latitude:35.6746,longitude:139.75};
 const map=(await setup({roundTrip:true,preview:async()=>({points:[a,bend,b],lengthM:380,returnPath:{points:[b,reverseBend,a],lengthM:460}})}))!;
 map.from();map.to();await screen.findByText('復路 460 m');
 const out=screen.getByLabelText('バス系統のプレビュー').getAttribute('d');
 const back=screen.getByLabelText('バス系統の復路プレビュー').getAttribute('d');
 const middle=(d:string|null)=>[...(d??'').matchAll(/-?\d+(?:\.\d+)?/g)][3]?.[0];
 expect(middle(out)).toBeDefined();
 expect(middle(back)).not.toBe(middle(out)); // the native result is not a synthetic reversal
 const preview=vi.mocked(invoke).mock.calls.find(c=>c[0]==='preview_gameplay_ofm_bus')?.[1];
 expect(preview).toMatchObject({path:{pattern:'openRoundTrip',respectOneway:false}});
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
 fireEvent.click(screen.getByRole('button',{name:'バス系統を確定'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('put_gameplay_ofm_bus',preview));
});

test.each([false,true])('can choose mapped one-way restrictions for a new route (waypoints = %s)',async waypoints=>{
 const preview=async()=>({points:[a,b],lengthM:380,returnPath:{points:[b,waypoint,a],lengthM:440}});
 const {from,to}=(await setup({waypoints,roundTrip:true,autoStart:false,preview}))!;
 const mode=screen.getByRole<HTMLInputElement>('checkbox',{name:'地図データの一方通行に従う'});
 expect(mode.checked).toBe(false);
 fireEvent.click(mode);
 expect(mode.checked).toBe(true);
 fireEvent.click(screen.getByRole('button',{name:'新しいバス系統'}));
 expect(screen.queryByRole('checkbox',{name:'地図データの一方通行に従う'})).toBeNull();
 expect(screen.getByText('地図データの一方通行に従います')).toBeTruthy();
 from();to();await screen.findByText('復路 440 m');
 const request=vi.mocked(invoke).mock.calls.find(c=>c[0]==='preview_gameplay_ofm_bus')?.[1];
 expect(request).toMatchObject({path:{pattern:'openRoundTrip',respectOneway:true}});
 fireEvent.click(screen.getByRole('button',{name:'バス系統を確定'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('put_gameplay_ofm_bus',request));
});

test.each([false,true])('an unavailable native return cannot be committed as outbound only (one-way = %s)',async respectOneway=>{
 const map=(await setup({roundTrip:true,autoStart:false,preview:async()=>({points:[a,b],lengthM:380,
  returnUnavailableReason:'終点から起点に戻れる道路がありません。'})}))!;
 if(respectOneway)fireEvent.click(screen.getByRole('checkbox',{name:'地図データの一方通行に従う'}));
 fireEvent.click(screen.getByRole('button',{name:'新しいバス系統'}));
 map.from();map.to();await screen.findByText(/終点から起点に戻れる道路がありません/);
 expect(invoke).toHaveBeenCalledWith('preview_gameplay_ofm_bus',expect.objectContaining({
  path:expect.objectContaining({respectOneway}),
 }));
 expect(screen.getByLabelText('バス系統のプレビュー')).toBeTruthy();
 expect(screen.queryByLabelText('バス系統の復路プレビュー')).toBeNull();
 const confirm=screen.getByRole<HTMLButtonElement>('button',{name:'バス系統を確定'});
 expect(confirm.disabled).toBe(true);fireEvent.click(confirm);
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
});

test('legacy open line edit preserves the absent return marker after engine upgrade',async()=>{
 const map=(await setup({multiple:true,roundTrip:true,autoStart:false,preview:async()=>({points:[a,b],lengthM:380})}))!;
 const select=screen.getByRole<HTMLSelectElement>('combobox',{name:'表示するバス系統'});
 fireEvent.change(select,{target:{value:'owned-b'}});
 fireEvent.click(screen.getByRole('button',{name:'バス系統を引き直す'}));
 map.from();map.to();await screen.findByText('道路経路 380 m・停留所2か所');
 const preview=vi.mocked(invoke).mock.calls.find(c=>c[0]==='preview_gameplay_ofm_bus')?.[1];
 expect(preview).toMatchObject({routeId:'owned-b'});
 expect(preview).not.toHaveProperty('path.pattern');
});

test('transport operation capability cannot bypass an incomplete provider evidence batch',async()=>{
 const map=(await setup({rectangle:true,roundTrip:true,operation:true,
  preview:async()=>{throw Error('a missing provider tile must prevent this native request');}}))!;
 const all=await observe.getMockImplementation()!([[14551,6452],[14552,6452]],'r1');
 observe.mockResolvedValueOnce(all.slice(1));
 map.from();map.to();
 await screen.findByText('範囲内の道路データが揃っていません。');
 expect(read).not.toHaveBeenCalled();
 expect(invoke).not.toHaveBeenCalledWith('preview_gameplay_ofm_bus',expect.anything());
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
 expect(invoke).not.toHaveBeenCalledWith('configure_bus_operation',expect.anything());
 expect(screen.queryByRole('button',{name:'バス系統を確定'})).toBeNull();
});


test('waypoint-capable ordinary Bus appends, moves, removes and locally undoes controls without mutating',async()=>{
 const requests=()=>vi.mocked(invoke).mock.calls.filter(c=>c[0]==='preview_gameplay_ofm_bus');
 const {map,pressPoint}=(await setup({waypoints:true,preview:async()=>{
  const req=requests().at(-1)![1] as {path:{end:{vertex:number};via?:{vertex:{vertex:number}}[]}};
  const vertices=[a,b,waypoint,waypointMoved];
  const via=req.path.via?.map(c=>vertices[c.vertex.vertex])??[];
  return {points:[a,...via,vertices[req.path.end.vertex]],lengthM:400+via.length,...(via.length?{viaPointIndices:[1]}:{})};
 }}))!;
 pressPoint(a);pressPoint(waypoint);await screen.findByText('道路経路 400 m・停留所2か所');
 pressPoint(b);await screen.findByText('道路経路 401 m・停留所2か所');
 const handle=screen.getByLabelText('バスの経由点 1');
 const p=project(waypoint.latitude,waypoint.longitude,{latitude:a.latitude,longitude:139.75,zoom:16});
 const q=project(waypointMoved.latitude,waypointMoved.longitude,{latitude:a.latitude,longitude:139.75,zoom:16});
 const ptr=(x:number,y:number)=>({pointerId:7,isPrimary:true,button:0,clientX:x,clientY:y});
 fireEvent.pointerDown(handle,ptr(p.x,p.y));fireEvent.pointerMove(map,ptr(q.x,q.y));fireEvent.pointerUp(map,ptr(q.x,q.y));
 await waitFor(()=>expect(requests().at(-1)?.[1]).toMatchObject({path:{via:[{vertex:{vertex:3}}]}}));
 await screen.findByText('道路経路 401 m・停留所2か所');
 fireEvent.keyDown(window,{key:'z',ctrlKey:true});
 expect(Number(screen.getByLabelText('バスの経由点 1').getAttribute('cx'))).toBeCloseTo(p.x);
 fireEvent.keyDown(window,{key:'Z',ctrlKey:true,shiftKey:true});
 expect(Number(screen.getByLabelText('バスの経由点 1').getAttribute('cx'))).toBeCloseTo(q.x);
 pressPoint(waypointMoved,1,screen.getByLabelText('バスの経由点 1'));
 await screen.findByText('道路経路 400 m・停留所2か所');
 expect(screen.queryByLabelText('バスの経由点 1')).toBeNull();
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
 expect(vi.mocked(invoke).mock.calls.some(([cmd])=>cmd.includes('undo'))).toBe(false);
 fireEvent.keyDown(window,{key:'Escape'});
 expect(screen.getByRole('button',{name:'新しいバス系統'})).toBeTruthy();
});

test('double-click binds completion to the new verified proof, not the old two-control preview',async()=>{
 let latest!: (value:unknown)=>void;
 let previews=0;
 const {pressPoint}=(await setup({waypoints:true,preview:async()=>{
  if(++previews===1)return {points:[a,waypoint],lengthM:235};
  return new Promise(resolve=>{latest=resolve;});
 }}))!;
 pressPoint(a);pressPoint(waypoint);await screen.findByText('道路経路 235 m・停留所2か所');
 pressPoint(b,1);pressPoint(b,2);
 await waitFor(()=>expect(previews).toBe(2));
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
 latest({points:[a,waypoint,b],lengthM:470,viaPointIndices:[1]});
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.objectContaining({path:expect.objectContaining({
   start:{feature:0,line:0,vertex:0},end:{feature:0,line:0,vertex:1},
   via:[{id:expect.any(String),tileKey:key,vertex:{feature:0,line:0,vertex:2}}],
 })})));
 expect(vi.mocked(invoke).mock.calls.filter(([cmd])=>cmd==='put_gameplay_ofm_bus')).toHaveLength(1);
});

test.each([1,2])('fails closed on a refused/malformed native leg index %d and never saves previous path',async failedLegIndex=>{
 let previews=0;
 const {pressPoint}=(await setup({waypoints:true,preview:async()=>++previews===1?
   {points:[a,waypoint],lengthM:235}:{points:[],lengthM:0,failedLegIndex}}))!;
 pressPoint(a);pressPoint(waypoint);await screen.findByText('道路経路 235 m・停留所2か所');
 pressPoint(b);fireEvent.keyDown(window,{key:'Enter'});
 await screen.findByText(failedLegIndex===1?'この区間を道路でつなげません。':'経路の確認結果が一致しません。位置を選び直してください。');
 expect(screen.queryAllByLabelText('接続できないバス区間の端')).toHaveLength(failedLegIndex===1?2:0);
 expect(screen.queryByLabelText('バス系統のプレビュー')).toBeNull();
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
});


test('existing bus opens edit without extending; deliberate endpoint click extends and first Escape exits extension only',async()=>{
 const name='既存 A',request=()=>vi.mocked(invoke).mock.calls.filter(c=>c[0]==='preview_gameplay_ofm_bus');
 const {map,pressPoint}=(await setup({waypoints:true,multiple:true,savedProof:true,autoStart:false,preview:async()=>{
  const req=request().at(-1)![1] as {path:{end:{vertex:number};via?:{vertex:{vertex:number}}[]}};
  const vertices=[a,b,waypoint,waypointMoved],v=req.path.via?.map(w=>vertices[w.vertex.vertex])??[];
  if(v.includes(waypointMoved))return {points:[],lengthM:0,failedLegIndex:1};
  return {points:[a,...v,vertices[req.path.end.vertex]],lengthM:400+v.length,...(v.length?{viaPointIndices:v.map((_,n)=>n+1)}:{})};
 }}))!;
 fireEvent.click(screen.getByRole('button',{name:'バス系統を引き直す'}));
 await screen.findByText('道路経路 400 m・停留所2か所');
 expect(request()[0][1]).toMatchObject({routeId:'owned-a',rawEvidence:null,tileBinding:null});
 pressPoint(waypoint);
 expect(request()).toHaveLength(1);
 const drag=(target:Element,to:typeof a)=>{
  const cam={latitude:a.latitude,longitude:139.75,zoom:16};
  const from=project(bend.latitude,bend.longitude,cam),dest=project(to.latitude,to.longitude,cam);
  const ptr=(x:number,y:number)=>({pointerId:11,isPrimary:true,button:0,clientX:x,clientY:y});
  fireEvent.pointerDown(target,ptr(from.x,from.y));fireEvent.pointerMove(map,ptr(dest.x,dest.y));fireEvent.pointerUp(map,ptr(dest.x,dest.y));
 };
 drag(screen.getByLabelText('バス経路の編集区間 1'),waypointMoved);
 await screen.findByText('この区間を道路でつなげません。');
 expect(screen.getByLabelText('バス系統のプレビュー')).toBeTruthy();
 expect((screen.getByRole('button',{name:'バス系統を確定'}) as HTMLButtonElement).disabled).toBe(true);
 drag(screen.getByLabelText('バス経路の編集区間 1'),waypoint);
 await screen.findByText('道路経路 401 m・停留所2か所');
 pressPoint(b,1,screen.getByLabelText('バスの終点'));
 await screen.findByText(/道路を順にクリック/);
 pressPoint(waypointMoved);
 await waitFor(()=>expect(request().at(-1)?.[1]).toMatchObject({path:{end:{vertex:3},via:[{vertex:{vertex:2}},{vertex:{vertex:1}}]}}));
 await screen.findByText('道路経路 402 m・停留所2か所');
 fireEvent.keyDown(window,{key:'Escape'});
 expect(screen.getByText(/線や点をドラッグして編集/)).toBeTruthy();
 expect((screen.getByRole('textbox',{name:'バス系統名'}) as HTMLInputElement).value).toBe(name);
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
 fireEvent.keyDown(window,{key:'Escape'});
 expect(screen.queryByRole('textbox',{name:'バス系統名'})).toBeNull();
 expect(screen.getByRole('button',{name:'バス系統を引き直す'})).toBeTruthy();
});

test('Escape and tool change retire an outstanding waypoint proof even after double-click finish intent',async()=>{
 let resolve!:(value:unknown)=>void;
 const {pressPoint}=(await setup({waypoints:true,preview:()=>new Promise(r=>{resolve=r;})}))!;
 pressPoint(a);pressPoint(b);pressPoint(b,2);
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('preview_gameplay_ofm_bus',expect.anything()));
 fireEvent.keyDown(window,{key:'Escape'});
 resolve({points:[a,b],lengthM:390});
 await screen.findByRole('button',{name:'新しいバス系統'});
 expect(screen.queryByLabelText('バス系統のプレビュー')).toBeNull();
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
});

test('hover previews already cached roadway after coalescing without requesting evidence or saving',async()=>{
 const requests=()=>vi.mocked(invoke).mock.calls.filter(c=>c[0]==='preview_gameplay_ofm_bus');
 const {map,pressPoint}=(await setup({waypoints:true,preview:async()=>{
  const req=requests().at(-1)![1] as {path:{end:{vertex:number};via?:{vertex:{vertex:number}}[]}};
  const vertices=[a,b,waypoint,waypointMoved];
  const via=req.path.via?.map(c=>vertices[c.vertex.vertex])??[];
  return {points:[a,...via,vertices[req.path.end.vertex]],lengthM:402,...(via.length?{viaPointIndices:[1]}:{})};
 }}))!;
 const cam={latitude:a.latitude,longitude:139.75,zoom:16};
 pressPoint(a);pressPoint(waypoint);await screen.findByText('道路経路 402 m・停留所2か所');
 const point=project(b.latitude,b.longitude,cam);
 for(let i=0;i<15;i++)fireEvent.pointerMove(map,{clientX:point.x+i/2,clientY:point.y,buttons:0});
 await screen.findByLabelText('カーソル位置までのバス経路');
 expect(requests()).toHaveLength(2);
 expect(read).toHaveBeenCalledTimes(1);
 expect(observe).not.toHaveBeenCalled();
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
 fireEvent.pointerLeave(map);
 expect(screen.queryByLabelText('カーソル位置までのバス経路')).toBeNull();
});


test('ordinary buses place and rename an on-route stop; no world writes before explicit save',async()=>{
 const occurrenceKey='a'.repeat(64),requests=()=>vi.mocked(invoke).mock.calls.filter(c=>c[0]==='preview_gameplay_ofm_bus');
 const {pressPoint}=(await setup({waypoints:true,interiorStops:true,multiple:true,savedProof:true,autoStart:false,preview:async()=>{
  const req=requests().at(-1)![1] as {requestStopTargets?:boolean;path:{interiorStops?:{id:string;name:string;fractionQ:number}[]}};
  return {points:[a,b],lengthM:370,stopOccurrenceKeys:[occurrenceKey],interiorStops:req.path.interiorStops?.map(stop=>({
   id:stop.id,stopId:'native:'+stop.id,name:stop.name,servesReturn:false,
   point:{latitude:a.latitude,longitude:a.longitude+(b.longitude-a.longitude)*stop.fractionQ/1_000_000},
  }))};
 }}))!;
 fireEvent.click(screen.getByRole('button',{name:'停留所を編集'}));
 await screen.findByLabelText('停留所を置くバス経路');
 expect(requests()[0][1]).toMatchObject({expectedWorldToken:'world-exact',routeId:'owned-a',requestStopTargets:true,rawEvidence:null});
 pressPoint({latitude:a.latitude,longitude:139.75});
 const field=await screen.findByRole('textbox',{name:'停留所名 1'});
 expect(screen.getByLabelText('停留所 1')).toBeTruthy();
 fireEvent.change(field,{target:{value:'中央'}});
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
 fireEvent.click(screen.getByRole('button',{name:'停留所の編集を確定'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.objectContaining({
  expectedWorldToken:'world-exact',routeId:'owned-a',path:expect.objectContaining({interiorStops:[
   {id:expect.any(String),name:'中央',occurrenceKey,fractionQ:500000},
  ]}),
 })));
 const put=vi.mocked(invoke).mock.calls.find(c=>c[0]==='put_gameplay_ofm_bus')?.[1];
 expect(put).not.toHaveProperty('requestStopTargets');
});

test('an intermediate stop moves on its confirmed route without changing its identity',async()=>{
 let holdMove=false,releaseMove:(()=>void)|undefined;
 const occurrenceKey='a'.repeat(64),requests=()=>vi.mocked(invoke).mock.calls.filter(c=>c[0]==='preview_gameplay_ofm_bus');
 const {map}=(await setup({waypoints:true,interiorStops:true,multiple:true,savedProof:true,autoStart:false,preview:async()=>{
  if(holdMove)await new Promise<void>(resolve=>{releaseMove=resolve;});
  const req=requests().at(-1)![1] as {path:{interiorStops?:{id:string;name:string;fractionQ:number}[]}};
  return {points:[a,b],lengthM:370,stopOccurrenceKeys:[occurrenceKey],interiorStops:req.path.interiorStops?.map(stop=>({
   id:stop.id,stopId:'native:'+stop.id,name:stop.name,servesReturn:false,
   point:{latitude:a.latitude,longitude:a.longitude+(b.longitude-a.longitude)*stop.fractionQ/1_000_000},
  }))};
 }}))!;
 fireEvent.click(screen.getByRole('button',{name:'停留所を編集'}));
 const route=await screen.findByLabelText('停留所を置くバス経路');
 const [x0,y0,x1,y1]=route.querySelector('path')!.getAttribute('d')!.match(/-?\d+(?:\.\d+)?/g)!.map(Number);
 const onRoute=(fraction:number,offset=0)=>fireEvent.click(map,{clientX:x0+(x1-x0)*fraction,clientY:y0+(y1-y0)*fraction+offset});
 onRoute(.25);await screen.findByRole('textbox',{name:'停留所名 1'});
 const oldMarker=screen.getByLabelText('停留所 1').getAttribute('cx');
 const one=(requests().at(-1)![1] as {path:{interiorStops:{id:string}[]}}).path.interiorStops[0].id;
 onRoute(.75);await screen.findByRole('textbox',{name:'停留所名 2'});
 fireEvent.change(screen.getByRole('textbox',{name:'停留所名 1'}),{target:{value:'中央'}});
 const before=requests().length;
 const selectMove=()=>fireEvent.click(screen.getByRole('button',{name:'停留所 1 の位置を変更'}));
 selectMove();
 onRoute(.5,30);
 expect(screen.getByText(/経路上の、駅や交差点から少し離れた位置/)).toBeTruthy();
 onRoute(.75);
 expect(screen.getByText('ここにはすでに停留所があります。')).toBeTruthy();
 expect(requests()).toHaveLength(before);
 fireEvent.keyDown(window,{key:'Escape'});
 expect(screen.queryByText(/停留所の移動先を/)).toBeNull();
 expect(screen.getByRole<HTMLInputElement>('textbox',{name:'停留所名 1'}).value).toBe('中央');
 holdMove=true;selectMove();onRoute(.5);
 await waitFor(()=>expect(releaseMove).toBeTypeOf('function'));
 fireEvent.keyDown(window,{key:'Escape'});
 holdMove=false;await act(async()=>releaseMove!());
 expect(screen.getByLabelText('停留所 1').getAttribute('cx')).toBe(oldMarker);
 expect(screen.queryByText(/停留所の移動先を/)).toBeNull();
 selectMove();onRoute(.5);
 await waitFor(()=>expect(screen.queryByText(/停留所の移動先を/)).toBeNull());
 expect(requests().at(-1)![1]).toMatchObject({expectedWorldToken:'world-exact',routeId:'owned-a',rawEvidence:null,requestStopTargets:true,
  path:{interiorStops:[{id:one,name:'中央',occurrenceKey,fractionQ:500000},{name:'停留所 2',fractionQ:750000}]}});
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
 fireEvent.click(screen.getByRole('button',{name:'停留所の編集を確定'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.objectContaining({routeId:'owned-a',
  path:expect.objectContaining({interiorStops:[{id:one,name:'中央',occurrenceKey,fractionQ:500000},
   expect.objectContaining({name:'停留所 2',fractionQ:750000})]})})));
});

test('overlapping stops require an explicit occurrence and Escape discards the unconfirmed stop draft',async()=>{
 const keys=['a','b','c'].map(s=>s.repeat(64)),requests=()=>vi.mocked(invoke).mock.calls.filter(c=>c[0]==='preview_gameplay_ofm_bus');
 const {pressPoint}=(await setup({waypoints:true,interiorStops:true,multiple:true,savedProof:true,autoStart:false,preview:async()=>{
  const req=requests().at(-1)![1] as {path:{interiorStops?:{id:string;name:string;fractionQ:number}[]}};
  return {points:[a,b,a,b],lengthM:1100,stopOccurrenceKeys:keys,interiorStops:req.path.interiorStops?.map(stop=>({
   id:stop.id,stopId:'native:'+stop.id,name:stop.name,servesReturn:false,
   point:{latitude:a.latitude,longitude:b.longitude+(a.longitude-b.longitude)*stop.fractionQ/1_000_000},
  }))};
 }}))!;
 fireEvent.click(screen.getByRole('button',{name:'停留所を編集'}));
 await screen.findByLabelText('停留所を置くバス経路');
 pressPoint({latitude:a.latitude,longitude:139.75});
 expect(screen.getByRole('group',{name:/この位置を経路が複数回通ります/})).toBeTruthy();
 expect(requests()).toHaveLength(1);
 fireEvent.click(screen.getByRole('button',{name:/通過 2/}));
 await screen.findByRole('textbox',{name:'停留所名 1'});
 expect(requests().at(-1)![1]).toMatchObject({path:{interiorStops:[{occurrenceKey:keys[1]}]}});
 fireEvent.keyDown(window,{key:'Escape'});
 expect(screen.getByRole('button',{name:'停留所を編集'})).toBeTruthy();
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
});


test.each(['auto','manual','stale'] as const)('new route %s stop policy only adds validated hints in the single explicit transaction',async policy=>{
 const occurrenceKey='a'.repeat(64),requests=()=>vi.mocked(invoke).mock.calls.filter(c=>c[0]==='preview_gameplay_ofm_bus');
 const {pressPoint}=(await setup({waypoints:true,interiorStops:true,autoStops:true,preview:async()=>{
  const req=requests().at(-1)![1] as {requestStopTargets?:boolean;requestAutoStopSuggestions?:boolean;path:{interiorStops?:{id:string;name:string;fractionQ:number}[]}};
  const proposed={id:'auto-1',name:'自動 1',occurrenceKey:policy==='stale'?'b'.repeat(64):occurrenceKey,fractionQ:500000};
  return {
   points:[a,b],lengthM:2300,...(req.requestStopTargets?{stopOccurrenceKeys:[occurrenceKey]}:{}),
   ...(req.requestAutoStopSuggestions?{autoStopSuggestions:[proposed]}:{}),
   ...(req.path.interiorStops?{interiorStops:[{id:proposed.id,name:proposed.name,stopId:'native-auto-1',
    point:{latitude:a.latitude,longitude:139.75},servesReturn:false}]}:{}),
  };
 }}))!;
 const chooser=screen.getByRole('combobox',{name:'停留所の配置'}) as HTMLSelectElement;
 expect(chooser.value).toBe('auto');
 if(policy==='manual')fireEvent.change(chooser,{target:{value:'manual'}});
 pressPoint(a);pressPoint(b);
 await screen.findByText(policy==='manual'?'道路経路 2300 m・停留所2か所':'道路経路 2300 m・途中停留所は確定時に配置');
 expect(requests()).toHaveLength(1);
 expect(requests()[0][1]).not.toHaveProperty('requestAutoStopSuggestions');
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
 fireEvent.click(screen.getByRole('button',{name:'バス系統を確定'}));
 if(policy==='stale'){
  await screen.findByText('停留所の自動配置を確認できません。');
  expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
  return;
 }
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything()));
 const put=vi.mocked(invoke).mock.calls.find(c=>c[0]==='put_gameplay_ofm_bus')?.[1];
 expect(put).not.toHaveProperty('requestAutoStopSuggestions');
 expect(put).not.toHaveProperty('requestStopTargets');
 if(policy==='manual'){
  expect(requests()).toHaveLength(1);
  expect(put).not.toHaveProperty('path.interiorStops');
 }else{
  expect(requests()).toHaveLength(3);
  expect(requests()[1][1]).toMatchObject({requestAutoStopSuggestions:true,requestStopTargets:true});
  expect(requests()[2][1]).toMatchObject({requestStopTargets:true,path:{interiorStops:[{id:'auto-1',occurrenceKey}]}});
  expect(put).toMatchObject({path:{interiorStops:[{id:'auto-1',occurrenceKey}]}});
 }
});


const loopAutomatic={id:'loop_fallback',stopId:'native-loop-fallback',name:'中間停留所',
 point:{latitude:a.latitude,longitude:139.75},servesReturn:false};
test('clicking the original start handle creates one circuit draft; only explicit confirmation writes',async()=>{
 const requests=()=>vi.mocked(invoke).mock.calls.filter(c=>c[0]==='preview_gameplay_ofm_bus');
 const {pressPoint}=(await setup({waypoints:true,closed:true,roundTrip:true,preview:async()=>{
  const req=requests().at(-1)![1] as {path:{pattern?:string}};
  if(req.path.pattern==='closedOneWay')return {points:[a,b,a],lengthM:760,viaPointIndices:[1],loopFallbackStop:loopAutomatic};
  return {points:[a,b],lengthM:380,returnPath:{points:[b,a],lengthM:380}};
 }}))!;
 pressPoint(a);pressPoint(b);await screen.findByLabelText('バス系統の復路プレビュー');
 pressPoint(a,1,screen.getByLabelText('バスの起点'));
 await screen.findByText('中間停留所（自動）');
 expect(screen.getAllByLabelText('バスの起終点')).toHaveLength(1);
 expect(screen.queryByLabelText('バスの終点')).toBeNull();
 expect(screen.queryByLabelText('バス系統の復路プレビュー')).toBeNull();
 expect(requests().at(-1)?.[1]).toMatchObject({path:{pattern:'closedOneWay',
  start:{vertex:0},end:{vertex:0},via:[{vertex:{vertex:1}}]}});
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
 fireEvent.click(screen.getByRole('button',{name:'バス系統を確定'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.objectContaining({
  path:expect.objectContaining({pattern:'closedOneWay',start:{feature:0,line:0,vertex:0},end:{feature:0,line:0,vertex:0}}),
 })));
});

test('escape invalidates pending closure; open endpoint click still finishes the non-loop as before',async()=>{
 let settle!:(reply:unknown)=>void;
 const requests=()=>vi.mocked(invoke).mock.calls.filter(c=>c[0]==='preview_gameplay_ofm_bus');
 const {pressPoint}=(await setup({waypoints:true,closed:true,preview:async()=>{
  const req=requests().at(-1)![1] as {path:{pattern?:string}};
  return req.path.pattern==='closedOneWay'?new Promise(resolve=>{settle=resolve;}):{points:[a,b],lengthM:380};
 }}))!;
 pressPoint(a);pressPoint(b);await screen.findByText('道路経路 380 m・停留所2か所');
 pressPoint(a,1,screen.getByLabelText('バスの起点'));
 await waitFor(()=>expect(settle).toBeTypeOf('function'));
 fireEvent.keyDown(window,{key:'Escape'});
 settle({points:[a,b,a],lengthM:760,viaPointIndices:[1],loopFallbackStop:loopAutomatic});
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
 fireEvent.click(screen.getByRole('button',{name:'新しいバス系統'}));
 pressPoint(a);pressPoint(b);await screen.findByText('道路経路 380 m・停留所2か所');
 pressPoint(b,1,screen.getByLabelText('バスの終点'));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything()));
 const put=vi.mocked(invoke).mock.calls.find(c=>c[0]==='put_gameplay_ofm_bus')?.[1];
 expect(put).not.toHaveProperty('path.pattern');
});

test('saved circuit reopens without extending; native fallback is read-only and yields to manual stop',async()=>{
 const keys=['a'.repeat(64),'b'.repeat(64)],requests=()=>vi.mocked(invoke).mock.calls.filter(c=>c[0]==='preview_gameplay_ofm_bus');
 const {pressPoint}=(await setup({waypoints:true,closed:true,interiorStops:true,multiple:true,savedProof:true,closedSaved:true,autoStart:false,preview:async()=>{
  const req=requests().at(-1)![1] as {requestStopTargets?:boolean;path:{interiorStops?:{id:string;name:string;fractionQ:number}[]}};
  return {points:[a,b,a],lengthM:760,viaPointIndices:[1],...(req.requestStopTargets?{stopOccurrenceKeys:keys}:{}),
   ...(req.path.interiorStops?.length?{interiorStops:req.path.interiorStops.map(s=>({...s,stopId:'native:'+s.id,servesReturn:false,
    point:{latitude:a.latitude,longitude:a.longitude+(b.longitude-a.longitude)*s.fractionQ/1_000_000}}))}:{loopFallbackStop:loopAutomatic}),
  };
 }}))!;
 fireEvent.click(screen.getByRole('button',{name:'バス系統を引き直す'}));
 await screen.findByText('中間停留所（自動）');
 expect(screen.getAllByLabelText('バスの起終点')).toHaveLength(1);
 pressPoint(waypoint);
 expect(requests()).toHaveLength(1);
 fireEvent.keyDown(window,{key:'Escape'});
 fireEvent.click(screen.getByRole('button',{name:'停留所を編集'}));
 await screen.findByText(/中間停留所（自動・追加すると置き換わります）/);
 expect(screen.queryByRole('textbox',{name:'停留所名 1'})).toBeNull();
 pressPoint({latitude:a.latitude,longitude:139.75});
 fireEvent.click(screen.getByRole('button',{name:/通過 1/}));
 await screen.findByRole('textbox',{name:'停留所名 1'});
 expect(screen.queryByText(/中間停留所（自動・追加すると置き換わります）/)).toBeNull();
 fireEvent.click(screen.getByRole('button',{name:'この停留所を外す'}));
 await screen.findByText(/中間停留所（自動・追加すると置き換わります）/);
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
});

test('new loop can still be drafted when its open prefix has no return; the prefix cannot be saved',async()=>{
 const requests=()=>vi.mocked(invoke).mock.calls.filter(c=>c[0]==='preview_gameplay_ofm_bus');
 const {pressPoint}=(await setup({waypoints:true,closed:true,roundTrip:true,preview:async()=>{
  const req=requests().at(-1)![1] as {path:{pattern?:string}};
  return req.path.pattern==='closedOneWay'?{points:[a,b,a],lengthM:760,viaPointIndices:[1],loopFallbackStop:loopAutomatic}:
   {points:[a,b],lengthM:380,returnUnavailableReason:'別方向の道路を通れません。'};
 }}))!;
 pressPoint(a);pressPoint(b);
 await screen.findByText(/復路を作れません/);
 expect((screen.getByRole('button',{name:'バス系統を確定'}) as HTMLButtonElement).disabled).toBe(true);
 fireEvent.keyDown(window,{key:'Enter'});
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
 pressPoint(a,1,screen.getByLabelText('バスの起点'));
 await screen.findByText('中間停留所（自動）');
 expect(requests().at(-1)?.[1]).toMatchObject({path:{pattern:'closedOneWay'}});
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
});


test('native observed-grade capability routes through proven structures without making a bridge vertex selectable',async()=>{
 const requests=()=>vi.mocked(invoke).mock.calls.filter(c=>c[0]==='preview_gameplay_ofm_bus');
 const {pressPoint}=(await setup({waypoints:true,observedGrade:true,preview:async()=>({points:[a,bridgeOnly,b],lengthM:575})}))!;
 pressPoint(bridgeOnly);
 expect(screen.getByText(/道路上の点を見つけられません/)).toBeTruthy();
 expect(screen.queryByLabelText('バスの起点')).toBeNull();
 expect(requests()).toHaveLength(0);
 pressPoint(a);pressPoint(b);
 await screen.findByText('道路経路 575 m・停留所2か所');
 const req=requests()[0][1];
 expect(req).toMatchObject({expectedWorldToken:'world-exact',path:{gradePolicy:'observedConnectors',
  start:{feature:0,line:0,vertex:0},end:{feature:0,line:0,vertex:1}}});
 expect(read).toHaveBeenCalledTimes(1);
 expect(screen.getByLabelText('バス系統のプレビュー').getAttribute('d')?.match(/\bL/g)).toHaveLength(2);
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
 fireEvent.click(screen.getByRole('button',{name:'バス系統を確定'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('put_gameplay_ofm_bus',req));
});

test.each([false,true])('saved road policies are preserved while editing (strict and grade = %s)',async retainedGrade=>{
 const requests=()=>vi.mocked(invoke).mock.calls.filter(c=>c[0]==='preview_gameplay_ofm_bus');
 const {pressPoint}=(await setup({waypoints:true,observedGrade:true,retainedGrade,retainedOneway:retainedGrade,multiple:true,savedProof:true,autoStart:false,preview:async()=>{
  const req=requests().at(-1)![1] as {path:{via?:unknown[];end:{vertex:number}}};
  return req.path.via?.length?{points:[a,b,waypoint],lengthM:660,viaPointIndices:[1]}:{points:[a,b],lengthM:380};
 }}))!;
 if(!retainedGrade)fireEvent.click(screen.getByRole('checkbox',{name:'地図データの一方通行に従う'}));
 fireEvent.click(screen.getByRole('button',{name:'バス系統を引き直す'}));
 await screen.findByText('道路経路 380 m・停留所2か所');
 pressPoint(b,1,screen.getByLabelText('バスの終点'));
 pressPoint(waypoint);
 await screen.findByText('道路経路 660 m・停留所2か所');
 expect(requests()).toHaveLength(2);
 for(const [,query] of requests()){
  expect(query).toHaveProperty('path.respectOneway',retainedGrade);
  if(retainedGrade)expect(query).toHaveProperty('path.gradePolicy','observedConnectors');
  else expect(query).not.toHaveProperty('path.gradePolicy');
 }
 expect(invoke).not.toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.anything());
});
