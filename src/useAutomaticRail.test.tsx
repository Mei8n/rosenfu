// @vitest-environment jsdom
import React from 'react';
import {act,cleanup,renderHook,render,screen,fireEvent,waitFor} from '@testing-library/react';
import {afterEach,beforeEach,expect,test,vi} from 'vitest';
import {useAutomaticRail} from './useAutomaticRail';
import {App} from './App';
import {invoke} from '@tauri-apps/api/core';
// These are rail interaction tests; road transport has separate acceptance tests.
vi.mock('./roadProvider',()=>({createRoadProvider:()=>({name:'unavailable test roads',load:async()=>{throw Error('road fixture unavailable');}})}));
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:()=>true}));
import type {RailRequest,RailResponse} from './realRailProvider';
const source={key:'a'.repeat(64)+':'+ 'b'.repeat(64),dataset_id:'trainsworld-realgraph-jp',revision:'2026-11-13',graph_sha256:'a'.repeat(64),source_sha256:'b'.repeat(64),source:'https://data.anita.garden/trainworld/2026-11-13/realgraph/jp.rgr',source_date:null,license:'ODbL-1.0',attribution:'© OpenStreetMap contributors',conversion_format_version:2};
const a={chain:7,position:.2,point:{longitude:139,latitude:35}},b={chain:8,position:.8,point:{longitude:139.002,latitude:35}};
const route={points:[a.point,{longitude:139.001,latitude:35.0001},b.point],spans:[{region:'jp',chain_id:7,from_fraction:.2,to_fraction:1},{region:'jp',chain_id:8,from_fraction:0,to_fraction:.8}]};
class FakeWorker{
 static instances:FakeWorker[]=[];messages:RailRequest[]=[];onmessage:((event:MessageEvent<RailResponse>)=>void)|null=null;onerror:(()=>void)|null=null;terminated=false;
 constructor(){FakeWorker.instances.push(this);}postMessage(message:RailRequest){this.messages.push(message);}terminate(){this.terminated=true;}reply(message:RailResponse){this.onmessage?.({data:message} as MessageEvent<RailResponse>);}
}
beforeEach(()=>{vi.useFakeTimers();FakeWorker.instances=[];vi.stubGlobal('Worker',FakeWorker);});
afterEach(()=>{cleanup();vi.useRealTimers();vi.unstubAllGlobals();vi.resetAllMocks();localStorage.clear();});
function setup(){const props={selected:true,enabled:true,canStart:true,worldEpoch:0,name:'reference route',bounds:{west:138.99,east:139.01,south:34.99,north:35.01},zoom:16,radius:12,onBuild:vi.fn(async()=>{})};const h=renderHook(p=>useAutomaticRail(p),{initialProps:props});const worker=FakeWorker.instances[0];return {...h,props,worker};}
function ready(h:ReturnType<typeof setup>){act(()=>h.worker.reply({id:0,kind:'loaded',source}));act(()=>vi.advanceTimersByTime(130));const request=h.worker.messages.at(-1)!;expect(request.kind).toBe('view');act(()=>h.worker.reply({id:request.id,kind:'view',view:{chains:[{id:7,points:[a.point,b.point]},{id:8,points:[a.point,b.point]}],truncated:false}}));act(()=>h.result.current.copy.begin());}
function pick(h:ReturnType<typeof setup>,end=false){act(()=>{void h.result.current.copy.click((end?b:a).point);});const request=h.worker.messages.at(-1)!;expect(request.kind).toBe('pick');act(()=>h.worker.reply({id:request.id,kind:'pick',pick:end?b:a,route:end?route:null}));}
test('automatically loads once; confirmation commits exactly the preview and source; ordinary rerenders retain it',async()=>{const h=setup();expect(h.result.current.status).toBe('loading');ready(h);pick(h);pick(h,true);const preview=h.result.current.copy.plan.build!;expect(preview.points).toEqual(route.points);expect(preview.provenance.osm_way_id).toBe('');expect(preview.provenance.network_path).toEqual(route.spans);await act(()=>h.result.current.copy.build());expect(h.props.onBuild).not.toHaveBeenCalled();h.rerender({...h.props,enabled:false});h.rerender(h.props);expect(h.result.current.copy.plan.build).toEqual(preview);expect(FakeWorker.instances).toHaveLength(1);act(()=>h.result.current.copy.setConfirmed(true));await act(()=>h.result.current.copy.build());expect(h.props.onBuild).toHaveBeenCalledExactlyOnceWith(preview);expect(h.result.current.copy.active).toBe(false);});
test('cancellation and actual world switch reject delayed results and terminate fetch/worker',()=>{const h=setup();ready(h);act(()=>{void h.result.current.copy.click(a.point);});const pending=h.worker.messages.at(-1)!;act(()=>h.result.current.copy.clear());act(()=>h.worker.reply({id:pending.id,kind:'pick',pick:a,route:null}));expect(h.result.current.copy.picks).toEqual([]);h.rerender({...h.props,worldEpoch:1});expect(h.worker.terminated).toBe(true);act(()=>h.worker.reply({id:0,kind:'loaded',source}));expect(h.result.current.status).toBe('loading');expect(h.result.current.copy.active).toBe(false);});
test('undo/redo restores exact graph identity after panning, never re-snaps',()=>{const h=setup();ready(h);pick(h);pick(h,true);const preview=h.result.current.copy.plan.build;act(()=>h.result.current.copy.setConfirmed(true));act(()=>h.result.current.copy.undo());expect(h.result.current.copy.confirmed).toBe(false);h.rerender({...h.props,bounds:{west:140,east:141,south:34,north:35}});act(()=>h.result.current.copy.redoPick());const request=h.worker.messages.at(-1)!;expect(request).toMatchObject({kind:'route',start:a,end:b});act(()=>h.worker.reply({id:request.id,kind:'pick',pick:b,route}));expect(h.result.current.copy.plan.build).toEqual(preview);});
test('failure differs from no nearby rail; retry and out-of-order view responses are guarded',()=>{const h=setup();act(()=>h.worker.reply({id:0,kind:'error',message:'HTTP 503'}));expect(h.result.current.status).toBe('failed');act(()=>h.result.current.retry());expect(h.worker.terminated).toBe(true);const current=FakeWorker.instances[1];act(()=>current.reply({id:0,kind:'loaded',source}));act(()=>vi.advanceTimersByTime(130));const old=current.messages.at(-1)!;h.rerender({...h.props,bounds:{...h.props.bounds,east:139.02}});act(()=>current.reply({id:old.id,kind:'view',view:{chains:[{id:99,points:[]}],truncated:false}}));expect(h.result.current.view.chains).toEqual([]);act(()=>vi.advanceTimersByTime(130));const request=current.messages.at(-1)!;act(()=>current.reply({id:request.id,kind:'view',view:{chains:[],truncated:false}}));act(()=>h.result.current.copy.begin());act(()=>{void h.result.current.copy.click(a.point);});const query=current.messages.at(-1)!;act(()=>current.reply({id:query.id,kind:'pick',pick:null,route:null}));expect(h.result.current.status).toBe('ready');expect(h.result.current.message).toContain('不存在は意味しません');});

test('App ordinary save preserves selected rail route; adding and building uses exact preview points',async()=>{
 vi.useRealTimers();vi.stubGlobal('PointerEvent',MouseEvent);
 const world={next_id:'3',players:[{id:'1',name:'Player'}],companies:[{id:'2',owner:'1',name:'Railway'}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
 vi.mocked(invoke).mockImplementation(async command=>command==='engine_info'?{name:'Rosenfu',version:'auto-rail-save-test'}:command==='list_save_slots'?{active_slot_id:'test',slots:[]}:command==='save_world'?{saved:true}:world);
 const view=render(<App/>);fireEvent.click(screen.getByText('自作サンプル（オフライン）',{selector:'button'}));await screen.findByText(/auto-rail-save-test/);
 fireEvent.click(screen.getByRole('button',{name:'実在の路線網に合わせる'}));const worker=FakeWorker.instances[0];expect(worker).toBeDefined();
 act(()=>worker.reply({id:0,kind:'loaded',source}));fireEvent.keyDown(window,{key:"+"});fireEvent.keyDown(window,{key:"+"});
 await waitFor(()=>expect(worker.messages.some(m=>m.kind==='view')).toBe(true));const request=worker.messages.filter(m=>m.kind==='view').at(-1)!;
 act(()=>worker.reply({id:request.id,kind:'view',view:{chains:[{id:7,points:route.points},{id:8,points:route.points}],truncated:false}}));
 fireEvent.click(screen.getByRole('button',{name:'実在線路の追従を開始'}));const map=view.container.querySelector('svg[role=img]')!;
 map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:1000,bottom:600,width:1000,height:600,toJSON:()=>({})});
 for(const [end,x] of [[false,500],[true,510]] as const){fireEvent.pointerDown(map,{clientX:x,clientY:300,button:0});fireEvent.pointerUp(map,{clientX:x,clientY:300});fireEvent.click(map,{clientX:x,clientY:300});const query=worker.messages.at(-1)!;expect(query.kind).toBe('pick');act(()=>worker.reply({id:query.id,kind:'pick',pick:end?b:a,route:end?route:null}));}
 const preview=view.container.querySelector('.reference-copy-preview')?.getAttribute('d');expect(preview).toBeTruthy();
 fireEvent.click(screen.getByText('保存',{selector:'button'}));await waitFor(()=>expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='save_world')).toBe(true));await screen.findByText(/選択中の参照候補は計画に追加するまで保存されません/);
 expect(worker.terminated).toBe(false);expect(FakeWorker.instances).toHaveLength(1);expect(view.container.querySelector('.reference-copy-preview')?.getAttribute('d')).toBe(preview);
 fireEvent.click(screen.getByLabelText('形状を確認した'));fireEvent.click(screen.getByRole('button',{name:'選択中の計画に追加'}));await waitFor(()=>expect(screen.queryByRole('button',{name:'選択中の計画に追加'})).toBeNull());
 fireEvent.click(screen.getByRole('button',{name:'実在の路線網に合わせる'}));expect(view.container.querySelectorAll('.automatic-rail-reference')).toHaveLength(0);expect(screen.getByRole('link',{name:'© OpenStreetMap contributors'}).closest('details')).toBeNull();
 fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));await waitFor(()=>expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='build_plan')).toBe(true));
 const payload=vi.mocked(invoke).mock.calls.find(([command])=>command==='build_plan')![1] as {alignments:{points:unknown;reference:{provenance:{network_path:unknown}}}[]};expect(payload.alignments[0].points).toEqual(route.points);expect(payload.alignments[0].reference.provenance.network_path).toEqual(route.spans);
});

test.each([true,false])('App retains water OSM attribution on sample background with or without derived rail (%s)',async linked=>{
 vi.useRealTimers();
 const points=[{latitude:35.67504,longitude:139.75},{latitude:35.676,longitude:139.751}];
 const world={next_id:'4',players:[{id:'1',name:'Player'}],companies:[{id:'2',owner:'1',name:'Railway'}],stations:[],tracks:[],routes:[],services:[],station_placements:[],source_records:[source],alignments:linked?[{id:'3',owner:'2',name:'Source rail',points,track_type:'normal',design_speed_kmh:120}]:[],alignment_provenance:linked?[{alignment_id:'3',start_point_index:0,end_point_index:1,source:{source_key:source.key,method:'rail_graph_route_v1',osm_way_id:'',mode:'rail',start_segment_index:0,end_segment_index:0,start_fraction:0,end_fraction:0,network_path:route.spans}}]:[]};
 vi.mocked(invoke).mockImplementation(async command=>command==='engine_info'?{name:'Rosenfu',version:'built-credit-test'}:command==='list_save_slots'?{active_slot_id:'test',slots:[]}:world);
 render(<App/>);fireEvent.click(screen.getByText('自作サンプル（オフライン）',{selector:'button'}));await screen.findByText(/built-credit-test/);
 const credits=screen.queryAllByRole('link',{name:'© OpenStreetMap contributors'});expect(credits).toHaveLength(1);expect(credits[0].closest('details')).toBeNull();
 expect(FakeWorker.instances).toHaveLength(0);
});

test('session history restores exact worker route synchronously and rejects stale, forged and old-epoch snapshots',()=>{
 const h=setup();ready(h);pick(h);const one=h.result.current.copy.sessionSnapshot;
 pick(h,true);const two=h.result.current.copy.sessionSnapshot,preview=h.result.current.copy.plan.build;
 act(()=>h.result.current.copy.setConfirmed(true));const messages=h.worker.messages.length;
 act(()=>expect(h.result.current.copy.restoreSession(one,two)).toBe(true));expect(h.result.current.copy.picks).toHaveLength(1);expect(h.result.current.copy.confirmed).toBe(false);
 act(()=>expect(h.result.current.copy.restoreSession(two,h.result.current.copy.sessionSnapshot)).toBe(true));expect(h.result.current.copy.plan.build).toEqual(preview);expect(h.worker.messages).toHaveLength(messages);
 act(()=>expect(h.result.current.copy.restoreSession(one,one)).toBe(false));
 act(()=>expect(h.result.current.copy.restoreSession({...two},h.result.current.copy.sessionSnapshot)).toBe(false));
 act(()=>{void h.result.current.copy.click(a.point);expect(h.result.current.copy.restoreSession(one,h.result.current.copy.sessionSnapshot)).toBe(false);});
 h.rerender({...h.props,worldEpoch:1});const worker=FakeWorker.instances.at(-1)!;act(()=>worker.reply({id:0,kind:'loaded',source}));
 act(()=>expect(h.result.current.copy.restoreSession(two,h.result.current.copy.sessionSnapshot)).toBe(false));expect(h.result.current.copy.active).toBe(false);
});
