// @vitest-environment jsdom
import React from 'react';
import {act,cleanup,fireEvent,render,renderHook,screen,waitFor,within} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {useReferenceCopy} from './useReferenceCopy';
import {syntheticReferenceGraph} from './fixtures/referenceGraph';
import {prepareReferenceCopy} from './referenceCopy';
import {readReferenceGraphBundle} from './referenceGraphBundle';
import {App} from './App';
import {invoke} from '@tauri-apps/api/core';
import {DEFAULT_CAMERA,project} from './map';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:()=>true}));
vi.mock('./referenceGraphBundle',()=>({readReferenceGraphBundle:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();vi.unstubAllGlobals();localStorage.clear();});
function fixture(){const graph=syntheticReferenceGraph();graph.edges[1].osmWayId='101';graph.edges[1].segmentIndex=1;graph.nodes[2].latitude=35.0005;return {graph,graphSha256:'a'.repeat(64),sourceSha256:'b'.repeat(64),source:{source:'synthetic',sourceDate:null,license:'ODbL-1.0',attribution:'© OpenStreetMap contributors'}};}
const start={latitude:35,longitude:134.9995},end={latitude:35.00025,longitude:135.0005};
function setup(){const props={bundle:fixture(),worldVersion:0,enabled:true,canStart:true,tool:'tracks',name:'参考線',onBuild:vi.fn(async()=>{})};const hook=renderHook(p=>useReferenceCopy(p),{initialProps:props});return {...hook,props};}
async function select(h:ReturnType<typeof setup>){act(()=>h.result.current.begin());await act(()=>h.result.current.click(start));act(()=>h.result.current.choose(h.result.current.candidates.findIndex(c=>c.edge.id==='101')));await act(()=>h.result.current.click(end));act(()=>h.result.current.choose(h.result.current.candidates.findIndex(c=>c.edge.id==='102')));}
test('copy preserves source coordinates and provenance, requires confirmation, and clears only after success',async()=>{const h=setup();await select(h);const expected=prepareReferenceCopy(h.props.bundle,h.result.current.picks[0],h.result.current.picks[1],'参考線');expect(h.result.current.plan.build).toEqual(expected);expect(expected.points).toHaveLength(3);await act(()=>h.result.current.build());expect(h.props.onBuild).not.toHaveBeenCalled();act(()=>h.result.current.setConfirmed(true));await act(()=>h.result.current.build());expect(h.props.onBuild).toHaveBeenCalledExactlyOnceWith(expected);expect(h.result.current.active).toBe(false);});
test('failed build preserves plan; undo/redo restores exactly and resets confirmation',async()=>{const h=setup();await select(h);const expected=h.result.current.plan.build;h.props.onBuild.mockRejectedValueOnce(Error('disk unavailable'));act(()=>h.result.current.setConfirmed(true));await act(()=>h.result.current.build());expect(h.result.current.plan.build).toEqual(expected);expect(h.result.current.message).toContain('disk unavailable');act(()=>h.result.current.undo());expect(h.result.current.picks).toHaveLength(1);expect(h.result.current.confirmed).toBe(false);act(()=>h.result.current.redoPick());expect(h.result.current.plan.build).toEqual(expected);expect(h.result.current.confirmed).toBe(false);});
test('replacement bundle and world change invalidate selections and delayed candidate results',async()=>{const h=setup();await select(h);act(()=>h.result.current.setConfirmed(true));h.rerender({...h.props,bundle:fixture()});expect(h.result.current.active).toBe(false);expect(h.result.current.picks).toEqual([]);act(()=>h.result.current.begin());let request:Promise<void>;act(()=>{request=h.result.current.click(start);h.result.current.clear();});await act(()=>request!);expect(h.result.current.candidates).toEqual([]);expect(h.result.current.active).toBe(false);act(()=>h.result.current.begin());h.rerender({...h.props,worldVersion:1});expect(h.result.current.active).toBe(false);});
const copyPanel=()=>within(document.querySelector<HTMLElement>('[aria-label="実在線路・道路に沿って敷設"]')!);
const empty={next_id:'3',players:[{id:'1',name:'Player'}],companies:[{id:'2',owner:'1',name:'Railway'}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
test('App drag does not select, explicit click choices preview the exact atomic build payload',async()=>{
 vi.stubGlobal('PointerEvent',MouseEvent);const bundle=fixture();const dx=DEFAULT_CAMERA.longitude-135,dy=DEFAULT_CAMERA.latitude-35;bundle.graph.nodes=bundle.graph.nodes.map(n=>({...n,latitude:n.latitude+dy,longitude:n.longitude+dx}));bundle.graph.dataset.bounds={west:DEFAULT_CAMERA.longitude-.01,east:DEFAULT_CAMERA.longitude+.01,south:DEFAULT_CAMERA.latitude-.01,north:DEFAULT_CAMERA.latitude+.01};
 vi.mocked(readReferenceGraphBundle).mockResolvedValue(bundle);vi.mocked(invoke).mockImplementation(async command=>command==='engine_info'?{name:'Rosenfu',version:'copy-test'}:empty);
 const view=render(<App/>);fireEvent.click(screen.getByText('自作サンプル（オフライン）',{selector:'button'}));await screen.findByText(/copy-test/);fireEvent.change(screen.getByLabelText('参考線バンドル3ファイル'),{target:{files:[new File(['x'],'graph.json')]}});await screen.findByText(/synthetic-test/);
 fireEvent.change(screen.getByLabelText('線路の種類'),{target:{value:'high_speed'}});fireEvent.click(copyPanel().getByRole('button',{name:'実在線路・道路に沿う敷設を開始'}));expect(screen.getByLabelText('線路の種類').matches(':disabled')).toBe(true);const map=view.container.querySelector('svg[role=img]')!;map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:1000,bottom:600,width:1000,height:600,toJSON:()=>({})});
 fireEvent.pointerDown(map,{clientX:500,clientY:300,button:0});fireEvent.pointerMove(map,{clientX:510,clientY:300});fireEvent.pointerUp(map,{clientX:510,clientY:300});fireEvent.click(map,{clientX:510,clientY:300});expect(copyPanel().queryAllByRole('button',{name:/候補 \d/})).toHaveLength(0);
 // Restore camera after the pan, then select with a complete normal pointer sequence.
 fireEvent.click(screen.getByText('東京へ戻る',{selector:'button'}));
 async function pick(p:typeof start,id:string){const point=project(p.latitude+dy,p.longitude+dx,DEFAULT_CAMERA);fireEvent.pointerDown(map,{clientX:point.x,clientY:point.y,button:0});fireEvent.pointerUp(map,{clientX:point.x,clientY:point.y});fireEvent.click(map,{clientX:point.x,clientY:point.y});await copyPanel().findByRole('button',{name:new RegExp('候補 .*way 101 / 区間 '+id)});fireEvent.click(copyPanel().getByRole('button',{name:new RegExp('候補 .*way 101 / 区間 '+id)}));}
 await pick(start,'0');await pick(end,'1');const preview=view.container.querySelector('.reference-copy-preview')?.getAttribute('d');expect(preview).toBeTruthy();fireEvent.click(copyPanel().getByLabelText('形状・出典を確認し、ゲーム内に新しい線路を敷設します'));fireEvent.click(copyPanel().getByRole('button',{name:'参考形状を明示着工'}));await waitFor(()=>expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='build_alignment_from_reference')).toBe(true));const payload=vi.mocked(invoke).mock.calls.find(([c])=>c==='build_alignment_from_reference')![1] as any;expect(payload.trackType).toBe('high_speed');expect(payload.points).toHaveLength(3);expect(payload.points[1]).toEqual({latitude:DEFAULT_CAMERA.latitude,longitude:DEFAULT_CAMERA.longitude});expect(payload.sourceRecord.graph_sha256).toBe('a'.repeat(64));expect(payload.provenance.method).toBe('osm_way_slice_v1');expect(preview).toBe(payload.points.map((p:any,i:number)=>{const projected=project(p.latitude,p.longitude,DEFAULT_CAMERA);return (i?'L':'M')+projected.x+','+projected.y;}).join(' '));await waitFor(()=>expect((screen.getByText('保存',{selector:'button'}) as HTMLButtonElement).disabled).toBe(false));fireEvent.click(screen.getByText('保存',{selector:'button'}));await waitFor(()=>expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='save_world')).toBe(true));
});

test('bundle replacement while candidate search is pending cannot publish old candidates',async()=>{const h=setup();act(()=>h.result.current.begin());let request:Promise<void>;act(()=>{request=h.result.current.click(start);});h.rerender({...h.props,bundle:fixture()});await act(()=>request!);expect(h.result.current.active).toBe(false);expect(h.result.current.candidates).toEqual([]);expect(h.result.current.picks).toEqual([]);expect(h.result.current.confirmed).toBe(false);});

test('reference plan station is session-only and included in atomic provenance-preserving build',async()=>{
 vi.stubGlobal('PointerEvent',MouseEvent);const bundle=fixture();const dx=DEFAULT_CAMERA.longitude-135,dy=DEFAULT_CAMERA.latitude-35;
 bundle.graph.nodes=bundle.graph.nodes.map(n=>({...n,latitude:n.latitude+dy,longitude:n.longitude+dx}));
 bundle.graph.dataset.bounds={west:DEFAULT_CAMERA.longitude-.01,east:DEFAULT_CAMERA.longitude+.01,south:DEFAULT_CAMERA.latitude-.01,north:DEFAULT_CAMERA.latitude+.01};
 vi.mocked(readReferenceGraphBundle).mockResolvedValue(bundle);
 vi.mocked(invoke).mockImplementation(async command=>command==='engine_info'?{name:'Rosenfu',version:'station-copy-test'}:empty);
 const view=render(<App/>);await screen.findByText(/station-copy-test/);
 fireEvent.change(screen.getByLabelText('参考線バンドル3ファイル'),{target:{files:[new File(['x'],'graph.json')]}});
 await screen.findByText(/synthetic-test/);fireEvent.click(copyPanel().getByRole('button',{name:'実在線路・道路に沿う敷設を開始'}));
 const map=view.container.querySelector('svg[role=img]')!;map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:1000,bottom:600,width:1000,height:600,toJSON:()=>({})});
 async function pick(p:typeof start,id:string){const point=project(p.latitude+dy,p.longitude+dx,DEFAULT_CAMERA);fireEvent.click(map,{clientX:point.x,clientY:point.y});fireEvent.click(await copyPanel().findByRole('button',{name:new RegExp('候補 .*way 101 / 区間 '+id)}));}
 await pick(start,'0');await pick(end,'1');
 fireEvent.click(screen.getByRole('tab',{name:'駅設置'}));fireEvent.change(screen.getByLabelText('ホーム長 (20m級・両相当)'),{target:{value:'3'}});
 fireEvent.click(map,{clientX:500,clientY:300});expect(map.querySelectorAll('.planned-station')).toHaveLength(1);
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='place_station_on_alignment'||c==='build_alignment_with_stations')).toBe(false);
 fireEvent.click(screen.getByRole('tab',{name:'線路'}));
 fireEvent.click(copyPanel().getByLabelText('形状・出典を確認し、ゲーム内に新しい線路を敷設します'));
 fireEvent.click(copyPanel().getByRole('button',{name:'参考形状を明示着工'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('build_alignment_with_stations',expect.objectContaining({stations:[expect.objectContaining({lengthM:60,widthM:12})],reference:expect.objectContaining({sourceRecord:expect.objectContaining({graph_sha256:'a'.repeat(64)})})})));
 await waitFor(()=>expect(map.querySelectorAll('.planned-station')).toHaveLength(0));
});
