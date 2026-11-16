// @vitest-environment jsdom
import React from 'react';
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {App} from './App';import {invoke} from '@tauri-apps/api/core';import {project} from './map';
import {createRoadDrawingEngine} from './roadDrawingEngine';
const mock=vi.hoisted(()=>({snapshot:null as any}));
vi.mock('./useAutomaticRoad',()=>({useAutomaticRoad:()=>({status:'ready',snapshot:mock.snapshot,message:'fixture',retry:vi.fn(),ensureCurrent:vi.fn(async()=>({status:'unavailable',reason:'fixture'}))})}));
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:()=>true}));
class RoadWorker {
 onmessage:any;onerror:any;engine:any;closed=false;
 postMessage(request:any){if(request.kind==='load'){this.engine=createRoadDrawingEngine(request.bundle,request.tileLines);return;}void this.engine(request.query).then((result:any)=>{if(!this.closed)this.onmessage?.({data:{id:request.id,result}});});}
 terminate(){this.closed=true;}
}
afterEach(()=>{cleanup();vi.clearAllMocks();vi.unstubAllGlobals();localStorage.clear();});
const longitude=139.76806640625,latitude=35.684,camera={latitude,longitude,zoom:16};
const world={view_state:camera,next_id:'3',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
async function start(snapshot:any=world){
 vi.stubGlobal('Worker',RoadWorker);
 const bundle={graph:{format:'rosenfu-reference-graph',version:1,dataset:{id:'t',revision:'r',source:'t',license:'t',bounds:{west:139,east:140,south:35,north:36}},nodes:[{osmId:'1',longitude:longitude-.002,latitude},{osmId:'2',longitude:longitude+.002,latitude}],edges:[{id:'1',osmWayId:'1',segmentIndex:0,from:'1',to:'2',mode:'road',layer:0,level:null,bridge:false,tunnel:false}]},roadWays:{'1':{highway:'residential',access:null,motorVehicle:null,structureUnsupported:false}},graphSha256:'a'.repeat(64),sourceSha256:'b'.repeat(64),source:{source:'t',sourceDate:null,license:'t',attribution:'t'}};
 mock.snapshot={bundle,roads:[]};
 vi.mocked(invoke).mockImplementation(async(command)=>{if(command==='engine_info')return {name:'Rosenfu',version:'road-app'};if(command==='list_save_slots')return {active_slot_id:'slot',slots:[]};if(command==='autosave_status')return {available:false};if(command==='save_world')return {saved:true};return snapshot;});
 render(<App/>);await screen.findByText(/road-app/);fireEvent.click(screen.getByRole('button',{name:'道路に合わせる'}));
 const map=document.querySelector('svg[role=img]')!;map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:1000,bottom:600,width:1000,height:600,toJSON:()=>({})});
 const click=async(offset:number,ctrlKey=false)=>{const p=project(latitude,longitude+offset,camera);await act(async()=>{fireEvent.click(map,{clientX:p.x,clientY:p.y,ctrlKey});await new Promise(r=>setTimeout(r,0));});};return {map,click};
}
test('road points form an editable continuous draft; undo and redo restore the exact road geometry',async()=>{
 const {click}=await start();await click(-.0008);await click(0);await click(.0008);
 const path=()=>document.querySelector('.confirmed-draft')?.getAttribute('d');const before=path();expect(before).toBeTruthy();expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
 fireEvent.keyDown(window,{key:'z',ctrlKey:true});expect(path()).not.toBe(before);fireEvent.keyDown(window,{key:'z',ctrlKey:true,shiftKey:true});expect(path()).toBe(before);
 fireEvent.keyDown(window,{key:'Escape'});expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
});
test('Ctrl temporarily uses ordinary placement without changing the road toggle',async()=>{const {click}=await start();await click(-.0008);await click(.0008,true);expect(document.querySelector('.confirmed-draft')).toBeTruthy();expect(screen.getByRole('button',{name:'道路に合わせる'}).getAttribute('aria-pressed')).toBe('true');expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);});

test('save stores normal editable geometry, reload retains it and permits continuation',async()=>{
 const h=await start();await h.click(-.0008);await h.click(0);const before=document.querySelector('.confirmed-draft')?.getAttribute('d');
 fireEvent.click(screen.getByRole('tab',{name:'セーブデータ'}));fireEvent.click(screen.getByRole('button',{name:'ワールドを保存'}));
 await waitFor(()=>expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='save_world')).toBe(true));
 const plans=(vi.mocked(invoke).mock.calls.find(([c])=>c==='save_world')![1] as any).constructionPlans;
 expect(plans.roadFollowing).toBe(false);expect(plans.draftEditedGeometry.line.length).toBeGreaterThan(1);expect(JSON.stringify(plans)).not.toContain('rawEvidence');
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='stage_ofm_construction_plan')).toBe(false);
 cleanup();const reloaded=await start({...world,construction_plans:plans});expect(document.querySelector('.confirmed-draft')?.getAttribute('d')).toBe(before);await reloaded.click(.0008);expect(document.querySelector('.confirmed-draft')?.getAttribute('d')).not.toBe(before);
});

test('road continuation connects exact built endpoints and preserves their original geometry',async()=>{
 const a={latitude,longitude:longitude-.0008},b={latitude,longitude:longitude+.0008};
 const source={id:'10',owner:'2',name:'source',points:[{latitude,longitude:longitude-.0015},a]},target={id:'11',owner:'2',name:'target',points:[b,{latitude,longitude:longitude+.0015}]};
 const snapshot={...world,alignments:[source,target]};const h=await start(snapshot);await h.click(-.0008);await h.click(.0008);
 expect(screen.getByRole('button',{name:'道路に合わせる'}).getAttribute('aria-pressed')).toBe('true');expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
 fireEvent.click(screen.getByRole('tab',{name:'セーブデータ'}));fireEvent.click(screen.getByRole('button',{name:'ワールドを保存'}));await waitFor(()=>expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='save_world')).toBe(true));
 const plans=(vi.mocked(invoke).mock.calls.find(([c])=>c==='save_world')![1] as any).constructionPlans;expect(plans.draftExtension).toEqual({alignmentId:'10',endpoint:'end'});expect(plans.draftDestination).toEqual({alignmentId:'11',endpoint:'start'});expect(plans.draftPoints[0]).toEqual(a);expect(plans.draftPoints.at(-1)).toEqual(b);expect(source.points[1]).toEqual(a);expect(target.points[0]).toEqual(b);
});
