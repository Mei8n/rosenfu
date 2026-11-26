// @vitest-environment jsdom
import React,{Profiler} from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen} from '@testing-library/react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {writeFileSync} from 'node:fs';
import {App} from './App';
import {panMap} from './mapTestNavigation';
import {settledMapClick} from './testMapClick';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();vi.unstubAllGlobals();});
const summarize=(values)=>{const sorted=[...values].sort((a,b)=>a-b);return {count:values.length,mean:values.reduce((a,b)=>a+b,0)/(values.length||1),p95:sorted[Math.max(0,Math.ceil(sorted.length*.95)-1)]??0};};
// Opt-in, deterministic input workload. No timing thresholds in the test suite.
// actualDuration measures React render work for each commit, not browser paint
// or commit-phase DOM mutation time. Separate elapsed values include jsdom.
test.skipIf(process.env.ROSENFU_PROFILE!=='1')('profile pointer frames, pan, zoom and plan edits at two world sizes',async()=>{
 const report=[];
 for(const lineCount of [1,16]){
  const alignments=Array.from({length:lineCount},(_,j)=>({id:String(j+100),owner:'2',name:'線路',points:Array.from({length:64},(_,i)=>({latitude:35.67504+j*.001,longitude:139.72+i*.001}))}));
  const world={next_id:'1000',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'会社'}],routes:[],services:[],alignments,stations:alignments.flatMap((a,j)=>Array.from({length:4},(_,i)=>({id:`s${j}-${i}`,owner:'2',name:`駅${j}-${i}`,latitude:a.points[0].latitude,longitude:139.73+i*.01}))),station_placements:alignments.flatMap((a,j)=>Array.from({length:4},(_,i)=>({station_id:`s${j}-${i}`,alignment_id:a.id,position:(i+1)/5,length_m:20,width_m:12})))};
  vi.stubGlobal('PointerEvent',MouseEvent);
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke).mockImplementation(async command=>{
   if(command==='engine_info')return {version:'profiler-test'};
   if(command==='world_snapshot')return world;
   if(command==='list_save_slots')return {active_slot_id:null,slots:[]};
   if(command==='water_mask_requests')return [];
   if(command==='preview_construction_plan')return {validationError:null};
   throw Error(command);
  });
  // Drive a 60Hz display manually, with four raw pointer events per frame.
  let frameId=0;const frames=new Map();
  vi.stubGlobal('requestAnimationFrame',(callback)=>{frames.set(++frameId,callback);return frameId;});
  vi.stubGlobal('cancelAnimationFrame',(id)=>frames.delete(id));
  const flushFrame=()=>act(()=>{const pending=[...frames.values()];frames.clear();for(const callback of pending)callback(performance.now());});
  let durations=[];
  const onRender=(_id,_phase,actualDuration)=>durations.push(actualDuration);
  render(<Profiler id="App" onRender={onRender}><App/></Profiler>);
  await screen.findByText(/profiler-test/);
  const map=document.querySelector('svg[role="img"]');
  map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:1000,bottom:600,width:1000,height:600,toJSON:()=>({})});
  const measure=async(name,operation)=>{
   for(let i=0;i<5;i++)await operation(i);
   durations=[];const elapsed=[],renderWork=[];
   for(let i=0;i<30;i++){const start=performance.now(),first=durations.length;await operation(i+5);elapsed.push(performance.now()-start);renderWork.push(durations.slice(first).reduce((a,b)=>a+b,0));}
   report.push({lineCount,points:lineCount*64,stations:lineCount*4,scenario:name,renderPerCommitMs:summarize(durations),renderPerOperationMs:summarize(renderWork),jsdomOperationMs:summarize(elapsed)});
  };
  await measure('pointer: 4 events/frame',()=>{for(let n=0;n<4;n++)fireEvent.pointerMove(map,{clientX:600+n,clientY:420+n});flushFrame();});
  await measure('pan',i=>panMap(i%2?8:-8,0));
  // Wheel-driven smooth zoom: flush enough frames for each transition to finish.
  await measure('zoom',async i=>{fireEvent.wheel(map,{clientX:500,clientY:300,deltaY:i%2?40:-40});for(let frame=0;frame<20;frame++)await act(async()=>{const pending=[...frames.values()];frames.clear();for(const callback of pending)callback(performance.now()+1000+frame*1000);});});
  for(const clientX of [180,300]){fireEvent.pointerDown(map,{clientX,clientY:400,button:0});fireEvent.pointerUp(map,{clientX,clientY:400,button:0});await settledMapClick(map,{clientX,clientY:400});}
  expect(map.querySelector('.confirmed-draft')).not.toBeNull();
  await measure('active preview: 4 events/frame',i=>{for(let n=0;n<4;n++)fireEvent.pointerMove(map,{clientX:350+i+n,clientY:410+n});flushFrame();});
  await measure('plan name edit',i=>{fireEvent.change(screen.getByLabelText('計画名（任意）'),{target:{value:`計画${i}`}});});
  expect(durations.length).toBeGreaterThan(0);
  cleanup();localStorage.clear();vi.unstubAllGlobals();
 }
 const output=JSON.stringify({environment:'React development Profiler / jsdom',samples:30,warmup:5,report},null,2);
 if(process.env.ROSENFU_PROFILE_OUTPUT)writeFileSync(process.env.ROSENFU_PROFILE_OUTPUT,output+'\n');
 console.info(output);
},120000);
