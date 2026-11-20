// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen,waitFor,within} from '@testing-library/react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
import {emptyConstructionPlans} from './constructionPlans';
import {LEVEL_LABELS} from './sectionInfrastructure';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
const points=[{latitude:35.67504,longitude:139.72},{latitude:35.67504,longitude:139.75},{latitude:35.67504,longitude:139.78}];
const line={id:'3',owner:'2',name:'旧線',points,anchor_indices:[0,1,2],track_type:'normal',infrastructure:[{track_count:1,level:'ground'},{track_count:1,level:'ground'}]};
const world={next_id:'4',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'旧会社'}],stations:[],routes:[],services:[],alignments:[line],station_placements:[],construction_plans:{...emptyConstructionPlans(),draftPoints:points,draftFinished:true,draftAnchorLevels:['ground','ground','ground']}};
const nextLine={...line,name:'新線',points:points.map(p=>({...p,latitude:p.latitude+.001}))};
const next={...world,alignments:[nextLine],companies:[{...world.companies[0],name:'新会社'}],construction_plans:{...world.construction_plans,draftPoints:nextLine.points,draftAnchorLevels:['ground','elevated_2','ground']}};
async function setup(){
 const state={fail:true,active:null as string|null};vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'height-world-test'};
  if(command==='world_snapshot')return world;
  if(command==='list_save_slots')return {active_slot_id:state.active,slots:[{id:'other',name:'別ワールド',loadable:true}]};
  if(command==='autosave_status')return {available:false};
  if(command==='preview_construction_plan')return world;
  if(command==='load_save_slot'){if(state.fail)throw Error('切替失敗');state.active='other';return next;}
  if(command==='update_alignment_anchor_height')return next;
  if(command==='save_world')return {saved:true};
  throw Error(command);
 });render(<App/>);await screen.findByText(/height-world-test/);await waitFor(()=>expect(invoke).toHaveBeenCalledWith('list_save_slots'));await act(async()=>{});return state;
}
async function loadOther(){
 fireEvent.click(screen.getByRole('button',{name:'読込'}));
 const picker=await screen.findByRole('dialog',{name:'ワールドを選択'});
 fireEvent.click(await within(picker).findByRole('button',{name:'別ワールド'}));
 const confirm=screen.queryByRole('button',{name:'読込を続ける'});if(confirm)fireEvent.click(confirm);
}
test('same-ID world switch clears built height selection; failed load keeps the old snapshot',async()=>{
 const state=await setup();fireEvent.click(screen.getByRole('tab',{name:'選択'}));fireEvent.click(screen.getByRole('button',{name:'建設済みアンカー 2 の高さを変更'}));
 await loadOther();await screen.findAllByText(/切替失敗/);
 expect(screen.getByRole('group',{name:'建設済みアンカーの高さ'})).toBeTruthy();
 expect((document.getElementById('company-name') as HTMLInputElement).value).toBe('旧会社');
 state.fail=false;fireEvent.click(within(screen.getByRole('dialog',{name:'ワールドを選択'})).getByRole('button',{name:'別ワールド'}));
 const confirm=screen.queryByRole('button',{name:'読込を続ける'});if(confirm)fireEvent.click(confirm);
 await screen.findByText('保存したワールドを読み込みました。');await act(async()=>{});
 expect(screen.queryByRole('group',{name:'建設済みアンカーの高さ'})).toBeNull();
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='update_alignment_anchor_height')).toBe(false);
 fireEvent.click(screen.getByRole('button',{name:'建設済みアンカー 2 の高さを変更'}));
 fireEvent.click(within(screen.getByRole('group',{name:'建設済みアンカーの高さ'})).getByRole('button',{name:LEVEL_LABELS.elevated}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('update_alignment_anchor_height',expect.objectContaining({expectedAlignment:nextLine})));
 expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==='update_alignment_anchor_height')).toHaveLength(1);
});
test('same-ID world switch clears planned height selection and does not edit the replacement plan',async()=>{
 const state=await setup();fireEvent.click(screen.getByRole('button',{name:'選択'}));fireEvent.click(document.querySelector('[data-anchor-index="1"][data-anchor-path="1"]')!);
 expect(screen.getByRole('group',{name:'アンカーの高さ'})).toBeTruthy();
 await loadOther();await screen.findAllByText(/切替失敗/);expect(screen.getByRole('group',{name:'アンカーの高さ'})).toBeTruthy();
 state.fail=false;fireEvent.click(within(screen.getByRole('dialog',{name:'ワールドを選択'})).getByRole('button',{name:'別ワールド'}));
 const confirm=screen.queryByRole('button',{name:'読込を続ける'});if(confirm)fireEvent.click(confirm);
 await screen.findByText('保存したワールドを読み込みました。');await act(async()=>{});
 expect(screen.queryByRole('group',{name:'アンカーの高さ'})).toBeNull();
 fireEvent.click(screen.getByRole('button',{name:'保存'}));await screen.findByText('ワールドと未着工計画を保存しました。');
 expect(invoke).toHaveBeenCalledWith('save_world',expect.objectContaining({constructionPlans:expect.objectContaining({draftAnchorLevels:['ground','elevated_2','ground'],draftPoints:nextLine.points})}));
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='update_alignment_anchor_height')).toBe(false);
});
