import {Profiler,type ProfilerOnRenderCallback} from 'react';
import {settledPointerMove,settledPointerEnter} from './testMapClick';
// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen} from '@testing-library/react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
import {GsiBackground} from './GsiBackground';
import {BuiltTrackLayer} from './BuiltTrackLayer';
import {StationLayer} from './StationLayer';
import {RouteLayer} from './RouteLayer';
import {TrackGeometryLayer} from './TrackRenderGeometry';
import {displayInfrastructureSections,schematicTrackPaths} from './sectionInfrastructure';
import {serviceBandPaths} from './serviceMapAppearance';
import {settledMapClick} from './testMapClick';
import {panMap} from './mapTestNavigation';

vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
// Instrument the real render functions inside React.memo, preserving the actual
// memo boundary and hooks. An extra memoized test wrapper would hide regressions.
vi.mock('./BuiltTrackLayer',async importOriginal=>{
 const actual=await importOriginal<typeof import('./BuiltTrackLayer')>();
 return {BuiltTrackLayer:{...actual.BuiltTrackLayer,type:vi.fn((actual.BuiltTrackLayer as unknown as {type:()=>unknown}).type)}};
});
vi.mock('./StationLayer',async importOriginal=>{
 const actual=await importOriginal<typeof import('./StationLayer')>();
 return {StationLayer:{...actual.StationLayer,type:vi.fn((actual.StationLayer as unknown as {type:()=>unknown}).type)}};
});
vi.mock('./RouteLayer',async importOriginal=>{
 const actual=await importOriginal<typeof import('./RouteLayer')>();
 return {RouteLayer:{...actual.RouteLayer,type:vi.fn((actual.RouteLayer as unknown as {type:()=>unknown}).type)}};
});
vi.mock('./TrackRenderGeometry',async importOriginal=>{
 const actual=await importOriginal<typeof import('./TrackRenderGeometry')>();
 return {...actual,TrackGeometryLayer:{...actual.TrackGeometryLayer,type:vi.fn((actual.TrackGeometryLayer as unknown as {type:()=>unknown}).type)}};
});
vi.mock('./GsiBackground',async importOriginal=>{
 const actual=await importOriginal<typeof import('./GsiBackground')>();
 return {...actual,GsiBackground:{...actual.GsiBackground,type:vi.fn((actual.GsiBackground as unknown as {type:()=>unknown}).type)}};
});
vi.mock('./sectionInfrastructure',{spy:true});
vi.mock('./serviceMapAppearance',{spy:true});
const renderSpy=(component:unknown)=>(component as {type:ReturnType<typeof vi.fn>}).type;
const layers=[BuiltTrackLayer,StationLayer,RouteLayer,TrackGeometryLayer,GsiBackground];
afterEach(()=>{cleanup();localStorage.clear();vi.clearAllMocks();vi.unstubAllGlobals();});

async function start(physical=false,onRender?:ProfilerOnRenderCallback){
 vi.stubGlobal('PointerEvent',MouseEvent);
 const points=Array.from({length:64},(_,i)=>({latitude:35.67504,longitude:139.72+i*.001}));
 const world={next_id:'1000',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'会社'}],routes:[],services:[{id:'40',owner:'2',name:'系統',start:'10'}],
  service_paths:[{service_id:'40',next_leg_key:'2',legs:[{key:'1',segment:{kind:'alignment',alignment_id:'3',from:'10',to:'29'}}],stops:[{kind:'start'},{kind:'on_leg',leg_key:'1',station_id:'29'}]}],
  alignments:[{id:'3',owner:'2',name:'線路',points,infrastructure:[{track_count:2,level:'ground'}]}],
  alignment_connections:physical?[{a:{alignment_id:'3',endpoint:'start'},b:{alignment_id:'3',endpoint:'end'}}]:[],
  stations:Array.from({length:20},(_,i)=>({id:String(i+10),owner:'2',name:`駅${i}`,latitude:35.67504,longitude:139.725+i*.002})),
  station_placements:Array.from({length:20},(_,i)=>({station_id:String(i+10),alignment_id:'3',position:(i+1)/22,length_m:20,width_m:12}))};
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='preview_construction_plan')return {validationError:null};
  if(command==='water_mask_requests')return [];
  if(command==='engine_info')return {version:'map-layer-test'};
  if(command==='world_snapshot')return world;
  if(command==='list_save_slots')return {active_slot_id:null,slots:[]};
  if(command==='preview_track_render_geometry')return {lanes:[{alignment_id:'3',section_index:0,lane_index:0,points}],movements:[]};
  if(command==='rename_company')return {...world,alignments:[{...world.alignments[0],points:points.map(p=>({...p,latitude:p.latitude+.001}))}],stations:world.stations.map(s=>({...s,name:`更新${s.name}`,latitude:s.latitude+.001}))};
  throw Error('unexpected '+command);
 });
 render(onRender?<Profiler id="App" onRender={onRender}><App/></Profiler>:<App/>);
 await screen.findByText(/map-layer-test/);
 if(physical)await vi.waitFor(()=>expect(document.querySelector('[data-physical-alignment]')).not.toBeNull());
 const map=document.querySelector<SVGSVGElement>('svg[role="img"]')!;
 map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:1000,bottom:600,width:1000,height:600,toJSON:()=>({})});
 return map;
}

for(const physical of [false,true])test(`cursor updates skip committed layers (${physical?'physical':'schematic'} tracks), camera and world changes update them`,async()=>{
 const map=await start(physical);
 // Station placement makes the cursor visibly change on every move.
 fireEvent.click(screen.getByRole('tab',{name:'駅設置'}));
 const initialTrack=map.querySelector('.alignment-path')!.getAttribute('d');
 const initialStation=map.querySelector('.station-marker')!.innerHTML;
 vi.clearAllMocks();
 const begin=performance.now();
 for(let i=0;i<10;i++)await settledPointerMove(map,{clientX:400+i*3,clientY:300+i});
 const elapsed=performance.now()-begin;
 const counts=layers.map(layer=>renderSpy(layer).mock.calls.length);
 console.info(JSON.stringify({scenario:physical?'physical':'schematic',pointerMoves:10,renderCounts:counts,infrastructureCalculations:vi.mocked(displayInfrastructureSections).mock.calls.length,schematicCalculations:vi.mocked(schematicTrackPaths).mock.calls.length,serviceBandCalculations:vi.mocked(serviceBandPaths).mock.calls.length,jsdomPointerMs:elapsed}));
 expect(counts).toEqual([0,0,0,0,0]);
 expect(displayInfrastructureSections).not.toHaveBeenCalled();
 expect(schematicTrackPaths).not.toHaveBeenCalled();
 expect(serviceBandPaths).not.toHaveBeenCalled();
 expect(document.querySelector('.station-placement-error')).not.toBeNull();
 panMap(40,20);
 expect(renderSpy(BuiltTrackLayer)).toHaveBeenCalled();
 expect(renderSpy(StationLayer)).toHaveBeenCalled();
 expect(renderSpy(RouteLayer)).toHaveBeenCalled();
 if(physical)expect(renderSpy(TrackGeometryLayer)).toHaveBeenCalled();
 expect(map.querySelector('.alignment-path')!.getAttribute('d')).not.toBe(initialTrack);
 expect(map.querySelector('.station-marker')!.innerHTML).not.toBe(initialStation);
 // Camera updates must not split world-space infrastructure again.
 expect(displayInfrastructureSections).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole('tab',{name:'設定'}));
 fireEvent.change(screen.getByLabelText('会社名'),{target:{value:'更新会社'}});
 vi.clearAllMocks();
 fireEvent.click(screen.getByRole('button',{name:'設定変更'}));
 await screen.findByText('更新駅0');
 expect(renderSpy(BuiltTrackLayer)).toHaveBeenCalled();
 expect(renderSpy(StationLayer)).toHaveBeenCalled();
 expect(renderSpy(RouteLayer)).toHaveBeenCalled();
 expect(displayInfrastructureSections).toHaveBeenCalled();
});

test('active construction preview and built hover leave committed layers untouched',async()=>{
 const map=await start();
 await settledMapClick(map,{clientX:180,clientY:380});
 await settledMapClick(map,{clientX:300,clientY:380});
 expect(map.querySelector('.confirmed-draft')).not.toBeNull();
 vi.clearAllMocks();
 for(let i=0;i<10;i++)await settledPointerMove(map,{clientX:350+i*3,clientY:390});
 expect(map.querySelector('.candidate-draft')).not.toBeNull();
 for(const layer of layers)expect(renderSpy(layer)).not.toHaveBeenCalled();
 expect(displayInfrastructureSections).not.toHaveBeenCalled();
 const hit=map.querySelector('.alignment-hit')!;
 await settledPointerEnter(hit,{clientX:500,clientY:300});
 expect(map.querySelector('.built-hover-point')).not.toBeNull();
 for(const layer of layers)expect(renderSpy(layer)).not.toHaveBeenCalled();
 fireEvent.pointerLeave(hit);
 expect(map.querySelector('.built-hover-point')).toBeNull();
});

test('visible service bands reuse geometry on hover and update on camera movement',async()=>{
 const map=await start();
 fireEvent.click(screen.getByRole('tab',{name:'系統'}));
 expect(map.querySelector('.service-map-section')).not.toBeNull();
 const before=map.querySelector('[data-service-id="40"]')!.getAttribute('d');
 vi.clearAllMocks();
 for(let i=0;i<10;i++)await settledPointerMove(map.querySelector('.alignment-hit')!,{clientX:500+i,clientY:300});
 expect(renderSpy(RouteLayer)).not.toHaveBeenCalled();
 expect(serviceBandPaths).not.toHaveBeenCalled();
 panMap(40,20);
 expect(serviceBandPaths).toHaveBeenCalled();
 expect(map.querySelector('[data-service-id="40"]')!.getAttribute('d')).not.toBe(before);
});

test('pointer bursts publish one React update per frame and cancel queued work on leave',async()=>{
 const commits:number[]=[];
 const map=await start(false,(_id,_phase,duration)=>commits.push(duration));
 let nextId=0;const frames=new Map<number,FrameRequestCallback>();
 vi.stubGlobal('requestAnimationFrame',(callback:FrameRequestCallback)=>{frames.set(++nextId,callback);return nextId;});
 vi.stubGlobal('cancelAnimationFrame',(id:number)=>frames.delete(id));
 commits.length=0;vi.clearAllMocks();
 for(let i=0;i<20;i++)fireEvent.pointerMove(map,{clientX:600+i,clientY:400+i});
 expect(commits).toHaveLength(0);
 expect(frames.size).toBe(1);
 act(()=>{const pending=[...frames.values()];frames.clear();for(const callback of pending)callback(16);});
 expect(commits).toHaveLength(1);
 for(const layer of layers)expect(renderSpy(layer)).not.toHaveBeenCalled();
 fireEvent.pointerMove(map,{clientX:640,clientY:440});
 expect(frames.size).toBe(1);
 fireEvent.pointerLeave(map);
 expect(frames.size).toBe(0);
});

test('a click before the visual frame uses the click coordinates and drops queued preview',async()=>{
 const map=await start();
 let callback:FrameRequestCallback|null=null;
 vi.stubGlobal('requestAnimationFrame',(next:FrameRequestCallback)=>{callback=next;return 1;});
 vi.stubGlobal('cancelAnimationFrame',()=>{callback=null;});
 fireEvent.pointerMove(map,{clientX:600,clientY:400});
 expect(callback).not.toBeNull();
 await settledMapClick(map,{clientX:180,clientY:380});
 expect(callback).toBeNull();
 const anchor=map.querySelector('.draft-anchor')!;
 expect(Number(anchor.getAttribute('x'))).toBeCloseTo(177.5);
 expect(Number(anchor.getAttribute('y'))).toBeCloseTo(377.5);
});
