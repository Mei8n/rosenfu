// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {App} from './App';

const {read,observe}=vi.hoisted(()=>({read:vi.fn(),observe:vi.fn()}));
vi.mock('@tauri-apps/api/core',()=>({isTauri:()=>true,invoke:vi.fn()}));
const a={latitude:35.67504,longitude:139.748},b={latitude:35.67504,longitude:139.752};
const bend={latitude:35.67532,longitude:139.75};
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
async function setup(config:{preview:()=>Promise<unknown>;ordinary?:boolean;rectangle?:boolean;multiple?:boolean;autoStart?:boolean;roundTrip?:boolean;operation?:boolean}){
 snapshots=config.rectangle?[adjacent,tile]:[tile];
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
  ...world,ofm_gameplay_bus_routes:{[first.id]:{},[second.id]:{},[foreign.id]:{}},
  road_transport:{nodes:[],edges:[{id:'native-edge',geometry:[a,bend,b]}],bus_stops:[],tram_sections:[],
   bus_routes:[first,second,foreign,nonordinary]},
 }:undefined;
 vi.mocked(invoke).mockImplementation(async (command,args)=>{
  if(command==='engine_info')return {version:'ordinary-test',road_transport_enabled:true,ordinary_ofm_bus_enabled:config.ordinary!==false,ordinary_ofm_bus_rectangle_enabled:config.rectangle===true,ordinary_ofm_bus_multiple_enabled:config.multiple===true,ordinary_ofm_bus_round_trip_enabled:config.roundTrip===true,operation_enabled:config.operation===true,bus_operation_enabled:config.operation===true};
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
 return {from:()=>press(config.rectangle?220.38:406.8),to:()=>press(593.2),map,pointerAt};
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

test('an unavailable native return explains the refusal and cannot be committed as outbound only',async()=>{
 const map=(await setup({roundTrip:true,preview:async()=>({points:[a,b],lengthM:380,
  returnUnavailableReason:'終点から起点に戻れる道路がありません。'})}))!;
 map.from();map.to();await screen.findByText(/終点から起点に戻れる道路がありません/);
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
