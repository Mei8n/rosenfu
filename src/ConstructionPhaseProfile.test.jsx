// @vitest-environment jsdom
import React,{Profiler} from 'react';
import {performance as nodePerformance} from 'node:perf_hooks';
import {writeFileSync} from 'node:fs';
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
import {emptyConstructionPlans} from './constructionPlans';
import {recordConstructionRender} from './constructionTiming';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
vi.mock('./roadProvider',()=>({createRoadProvider:()=>({name:'offline fixture',load:async()=>{throw Error('fixture offline');}})}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();vi.unstubAllGlobals();});

// Frontend phases only: IPC and the provider are mocked. Native cost is measured
// by the ignored Rust benchmarks, never inferred from this jsdom experiment.
test.skipIf(process.env.ROSENFU_CONSTRUCTION_PROFILE!=='1')('measure click, road wait, IPC and post-snapshot React phases',async()=>{
 const report=[];
 vi.stubGlobal('performance',nodePerformance);
 for(const [lineCount,roadCount] of [[20,100],[80,5000]]){
  for(let sample=0;sample<4;sample++){
   localStorage.setItem('rosenfu.construction-timing','1');nodePerformance.clearMeasures();
   const points=[{latitude:35.66,longitude:139.72},{latitude:35.66,longitude:139.74}];
   const lines=Array.from({length:lineCount},(_,i)=>({id:String(i+100),owner:'2',name:'Rail',track_type:'normal',points:points.map(p=>({...p,latitude:p.latitude+i*.0001}))}));
   const roads=Array.from({length:roadCount},(_,i)=>({id:`way/${i}`,points:[{latitude:35.65,longitude:139.72+i*.018/roadCount},{latitude:35.68,longitude:139.72+i*.018/roadCount}],layer:null,level:null,bridge:null,tunnel:null}));
   const plans={...emptyConstructionPlans(),draftPoints:points,draftFinished:true};
   const before={next_id:'10000',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],routes:[],services:[],stations:[],alignments:lines,construction_plans:plans,road_snapshots:[{revision:'fixture',source:'synthetic',source_url:'https://example.org/fixture',source_sha256:'a'.repeat(64),attribution:'Synthetic',license:'CC0',coverage_verified:true,road_coverage:[{west:139.7,south:35.6,east:139.8,north:35.8}],roads}]};
   const after={...before,next_id:'10001',construction_plans:emptyConstructionPlans(),alignments:[...lines,{id:'10000',owner:'2',name:'Built',points,track_type:'normal'}]};
   vi.mocked(isTauri).mockReturnValue(true);
   vi.mocked(invoke).mockImplementation(async command=>{
    if(command==='engine_info')return {version:'construction-phase-test'};
    if(command==='world_snapshot')return before;
    if(command==='list_save_slots')return {active_slot_id:null,slots:[]};
    if(command==='preview_construction_plan')return {validationError:null};
    if(command==='water_mask_requests'||command==='road_assessments')return [];
    if(command==='build_alignment')return after;
    throw Error(command);
   });
   const rootRenders=[];let collecting=false;
   const onRender=(...args)=>{if(collecting)rootRenders.push(args[2]);recordConstructionRender(...args);};
   render(<Profiler id="App" onRender={onRender}><App/></Profiler>);
   await screen.findByText(/construction-phase-test/);
   fireEvent.click(screen.getByRole('button',{name:'自作サンプル（オフライン）'}));
   collecting=true;const clickStart=nodePerformance.now();
   await act(async()=>{fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));});
   await waitFor(()=>expect(document.querySelectorAll('[data-alignment-id]')).toHaveLength(lineCount+1));
   await act(async()=>{await new Promise(resolve=>requestAnimationFrame(()=>resolve()));});
   const clickToObservedFrame=nodePerformance.now()-clickStart;collecting=false;
   if(process.env.ROSENFU_CONSTRUCTION_PROFILE_BASELINE!=='1')expect(nodePerformance.getEntriesByType('measure').some(e=>e.detail?.phase==='click-to-first-frame')).toBe(true);
   expect(document.querySelectorAll('[data-alignment-id]')).toHaveLength(lineCount+1);
   const phases={};
   for(const entry of nodePerformance.getEntriesByType('measure')){
    if(!entry.name.startsWith('rosenfu.construction.'))continue;
    const phase=entry.detail.phase;
    (phases[phase]??=[]).push(entry.duration);
   }
   if(sample>0)report.push({lineCount,roadCount,sample,rootRenders,clickToObservedFrame,phases});
   cleanup();localStorage.clear();
  }
 }
 const json=JSON.stringify({environment:'React development / jsdom; mocked IPC, offline provider',warmup:1,samples:3,report},null,2);
 if(process.env.ROSENFU_CONSTRUCTION_PROFILE_OUTPUT)writeFileSync(process.env.ROSENFU_CONSTRUCTION_PROFILE_OUTPUT,json+'\n');
 console.info(json);
},120000);
