// @vitest-environment jsdom
import React from 'react';
import {PbfWriter} from 'pbf';
import {cleanup,fireEvent,render,screen,waitFor,within} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {App} from './App';
import {DEFAULT_CAMERA,project} from './map';
import {decodeOpenFreeMapRoadTile,OFM_METADATA} from './openFreeMapRoadData';

const transport=vi.hoisted(()=>({fetch:vi.fn(),now:vi.fn()}));
vi.mock('@tauri-apps/api/core',()=>({isTauri:()=>true,invoke:vi.fn()}));
vi.mock('./roadProvider',async()=>{
 const {createOpenFreeMapRoadRepository}=await import('./openFreeMapRoadData');
 return {createRoadProvider:()=>({name:'OpenFreeMap / OpenStreetMap',
  ...createOpenFreeMapRoadRepository(transport.fetch,transport.now)})};
});
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();vi.unstubAllGlobals();});

function sourceTile(){
 const p=new PbfWriter();p.writeMessage(3,(_,l)=>{
  l.writeStringField(1,'transportation');l.writeVarintField(15,2);l.writeVarintField(5,4096);
  l.writeStringField(3,'class');l.writeMessage(4,(_,w)=>w.writeStringField(1,'minor'),null);
  l.writeMessage(2,(_,f)=>{f.writePackedVarint(2,[0,0]);f.writeVarintField(3,2);
   f.writePackedVarint(4,[9,850*2,400*2,10,700*2,0]);},null);
 },null);return p.finish();
}

test('normal Bus lets the player retry a failed real-provider request without losing their draft',async()=>{
 const raw=sourceTile(),revision='retry_fixture';
 const snap=await decodeOpenFreeMapRoadTile(raw,14552,6452,revision,0);
 const stops=snap.tileLines[0].coordinates.map(([longitude,latitude])=>({longitude,latitude}));
 const world={next_id:'10',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[],routes:[],services:[]};
 transport.now.mockReturnValue(0);
 let available=false;
 transport.fetch.mockImplementation(async (url:string)=>{
  if(url===OFM_METADATA)return available?new Response(JSON.stringify({maxzoom:14,
   tiles:[OFM_METADATA+'/'+revision+'/{z}/{x}/{y}.pbf']})):new Response(null,{status:503});
  return new Response(new Uint8Array(raw));
 });
 vi.mocked(invoke).mockImplementation(async (cmd,args)=>{
  if(cmd==='engine_info')return {name:'Rosenfu',version:'bus-source-retry',road_transport_enabled:true,ordinary_ofm_bus_enabled:true};
  if(cmd==='world_snapshot')return world;
  if(cmd==='list_save_slots')return {active_slot_id:null,slots:[]};
  if(cmd==='autosave_status')return {available:false};
  if(cmd==='ofm_world_token')return 'retry-exact-world';
  if(cmd==='preview_gameplay_ofm_bus')return {points:stops,lengthM:300};
  if(cmd==='put_gameplay_ofm_bus')return {...world,ofm_gameplay_bus:(args as {path:unknown}).path};
  throw Error('unexpected '+cmd);
 });
 render(<App/>);await screen.findByText(/bus-source-retry/);
 fireEvent.click(screen.getByRole('tab',{name:'バス'}));
 fireEvent.click(screen.getByRole('button',{name:'新しいバス系統'}));
 const name=screen.getByRole('textbox',{name:'バス系統名'});
 fireEvent.change(name,{target:{value:'再取得して続行'}});
 await screen.findByText(/HTTP 503/);
 const status=screen.getByRole('status',{name:'バスの道路データ状態'});
 expect(status.textContent).toContain('HTTP 503');
 expect(transport.fetch.mock.calls.filter(([url])=>url===OFM_METADATA)).toHaveLength(1);
 expect(vi.mocked(invoke).mock.calls.map(([cmd])=>cmd)).not.toContain('preview_gameplay_ofm_bus');
 // Repeated clicks during provider cooldown cannot trigger another HTTP request.
 fireEvent.click(within(status).getByRole('button',{name:'道路データを再取得'}));
 await screen.findByText(/道路タイル情報再試行待機中/);
 expect(transport.fetch.mock.calls.filter(([url])=>url===OFM_METADATA)).toHaveLength(1);
 available=true;transport.now.mockReturnValue(61_000);
 fireEvent.click(screen.getByRole('button',{name:'道路データを再取得'}));
 await waitFor(()=>expect(transport.fetch).toHaveBeenCalledWith(snap.bundle.source.source,expect.anything()));
 await waitFor(()=>expect(screen.queryByRole('status',{name:'バスの道路データ状態'})).toBeNull());
 expect(name).toHaveProperty('value','再取得して続行');
 const map=screen.getByRole('img',{name:/^駅配置用/});
 map.getBoundingClientRect=()=>new DOMRect(0,0,1000,600);
 vi.stubGlobal('PointerEvent',MouseEvent);
 for(const [index,a] of stops.entries()){
  const {x,y}=project(a.latitude,a.longitude,{...DEFAULT_CAMERA,zoom:16});
  const at={clientX:x,clientY:y,pointerId:1,isPrimary:true,button:0};
  fireEvent.pointerDown(map,at);fireEvent.pointerUp(map,at);fireEvent.click(map,at);
  if(index===0)await screen.findByText('道路上の終点を選択');
 }
 const key=snap.bundle.source.source,sha=snap.bundle.sourceSha256;
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('preview_gameplay_ofm_bus',expect.objectContaining({
  expectedWorldToken:'retry-exact-world',name:'再取得して続行',
  path:expect.objectContaining({tileKey:key,tileRawSha256:sha,
   start:{feature:0,line:0,vertex:0},end:{feature:0,line:0,vertex:1}}),
  rawEvidence:expect.objectContaining({sha256:sha,byteLength:raw.length}),
 })));
 const commands=()=>vi.mocked(invoke).mock.calls.map(([cmd])=>cmd);
 expect(commands()).not.toContain('put_gameplay_ofm_bus');
 fireEvent.click(await screen.findByRole('button',{name:'バス系統を確定'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('put_gameplay_ofm_bus',expect.objectContaining({
  expectedWorldToken:'retry-exact-world',name:'再取得して続行',
  tileBinding:expect.objectContaining({url:key,rawSha256:sha,revision}),
 })));
 expect(commands()).not.toContain('install_native_road_pilot');
});
