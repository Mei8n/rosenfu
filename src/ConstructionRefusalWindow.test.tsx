// @vitest-environment jsdom
import React from 'react';
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {App} from './App';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:()=>true}));
const world={next_id:'3',players:[{id:'1',name:'Player'}],companies:[{id:'2',owner:'1',name:'Railway'}],stations:[],alignments:[],routes:[],services:[],station_placements:[]};
afterEach(()=>{cleanup();vi.resetAllMocks();vi.unstubAllGlobals();localStorage.clear();});
async function start(preview:()=>Promise<unknown>){
 vi.stubGlobal('PointerEvent',MouseEvent);
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'refusal-window-test'};
  if(command==='world_snapshot')return world;
  if(command==='list_save_slots')return {active_slot_id:'test',slots:[]};
  if(command==='autosave_status')return {available:false};
  if(command==='preview_construction_plan')return preview();
  if(command==='save_world')return {saved:true};
  throw Error('unexpected '+command);
 });
 render(<App/>);fireEvent.click(screen.getByRole('button',{name:'自作サンプル（オフライン）'}));
 await screen.findByText(/refusal-window-test/);
 const map=screen.getByRole('img',{name:'駅配置用サンプル地図'});
 map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 fireEvent.click(map,{clientX:180,clientY:130});fireEvent.click(map,{clientX:320,clientY:130});
 return map;
}
test('native refusal uses the one positioned drawing window and clears when pointer leaves',async()=>{
 const map=await start(async()=>{throw '水域データの網羅性を確認できません。';});
 fireEvent.pointerMove(map,{clientX:360,clientY:130});
 await waitFor(()=>expect(document.querySelector('.drawing-tooltip')?.textContent).toContain('網羅性を確認できません'));
 const tip=document.querySelector<HTMLElement>('.drawing-tooltip')!;
 expect(document.querySelectorAll('.construction-tooltip')).toHaveLength(1);
 expect(tip.style.left).not.toBe('');expect(tip.style.top).not.toBe('');
 fireEvent.pointerLeave(map);expect(document.querySelectorAll('.construction-tooltip')).toHaveLength(0);
});
test('finished plan hover displays current backend refusal without an extra panel',async()=>{
 const map=await start(async()=>{throw '水面横断が30kmを超えています。';});
 fireEvent.keyDown(document.body,{key:'Escape'});
 const hit=map.querySelector('.plan-hover-hit')!;fireEvent.pointerEnter(hit,{clientX:250,clientY:130});
 await waitFor(()=>expect(document.querySelector('.plan-hover-tooltip')?.textContent).toContain('この計画は敷設不可：水面横断'));
 expect(document.querySelectorAll('.construction-tooltip')).toHaveLength(1);
 fireEvent.pointerLeave(hit);expect(document.querySelectorAll('.construction-tooltip')).toHaveLength(0);
});
test('old cursor refusal cannot overwrite a newer accepted hovered plan',async()=>{
 let rejectOld:(reason:unknown)=>void=()=>{};let calls=0;
 const map=await start(()=>{calls++;return calls===1?new Promise((_,reject)=>{rejectOld=reject;}):Promise.resolve(world);});
 fireEvent.pointerMove(map,{clientX:360,clientY:130});
 await waitFor(()=>expect(calls).toBe(1));
 fireEvent.keyDown(document.body,{key:'Escape'});
 fireEvent.pointerEnter(map.querySelector('.plan-hover-hit')!,{clientX:250,clientY:130});
 await waitFor(()=>expect(calls).toBe(2));await act(async()=>{});
 await act(async()=>rejectOld('古い水域拒否'));
 expect(document.querySelector('.plan-hover-tooltip')?.textContent).not.toContain('古い水域拒否');
 expect(document.querySelectorAll('.construction-tooltip')).toHaveLength(1);
});
test('a refused preview does not prevent draft save and a rejected build keeps the plan',async()=>{
 const reason='水域情報が不足しているため、この区間を判定できません。';
 const map=await start(async()=>{throw reason;});
 const prior=vi.mocked(invoke).getMockImplementation()!;
 vi.mocked(invoke).mockImplementation(async (command,args)=>{
  if(command==='build_alignment'||command==='build_plan')throw reason;
  return prior(command,args);
 });
 fireEvent.keyDown(document.body,{key:'Escape'});
 fireEvent.pointerEnter(map.querySelector('.plan-hover-hit')!,{clientX:250,clientY:130});
 await waitFor(()=>expect(document.querySelector('.plan-hover-tooltip')?.textContent).toContain(reason));
 fireEvent.click(screen.getByRole('button',{name:'保存'}));
 await waitFor(()=>expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='save_world')).toBe(true));
 const saved=vi.mocked(invoke).mock.calls.find(([c])=>c==='save_world')![1] as {constructionPlans:{draftPoints:unknown[]}};
 expect(saved.constructionPlans.draftPoints).toHaveLength(2);
 expect(saved).not.toHaveProperty('water_coverage');
 await screen.findByText('ワールドと未着工計画を保存しました。');
 fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));
 await waitFor(()=>expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='build_alignment'||c==='build_plan')).toBe(true));
 await waitFor(()=>expect(document.querySelector('.map-notices')?.textContent).toContain(reason));
 expect(map.querySelector('.plan-hover-hit')).not.toBeNull();
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
});

test('station placement keeps the existing native refusal status',async()=>{
 const reason='計画駅を含む区間の環境情報が不足しています。';
 await start(async()=>{throw reason;});
 fireEvent.keyDown(document.body,{key:'Escape'});
 fireEvent.click(screen.getByRole('tab',{name:'建設'}));
 fireEvent.click(screen.getByRole('tab',{name:'駅設置'}));
 await waitFor(()=>expect(document.querySelector('.construction-tooltip')?.textContent).toContain(reason));
});
