// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen,waitFor,within} from '@testing-library/react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
import {builtAnchorHeight,currentBuiltHeightSelection} from './BuiltAnchorHeight';
import {LEVEL_LABELS} from './sectionInfrastructure';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
const points=[{latitude:35.67504,longitude:139.72},{latitude:35.67504,longitude:139.75},{latitude:35.67504,longitude:139.78}];
const line={id:'3',owner:'2',name:'A',points,anchor_indices:[0,1,2],track_type:'normal',infrastructure:[{track_count:1 as const,level:'ground' as const},{track_count:1 as const,level:'ground' as const}]};
const world={next_id:'4',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[],routes:[],services:[],alignments:[line],station_placements:[]};
test('actual profile anchor height is not rounded to its nominal structure level',()=>{
 expect(builtAnchorHeight({...line,infrastructure:[],section_profiles:[{start_height_m:0,end_height_m:4.2},{start_height_m:4.2,end_height_m:0}]},1)).toBe(4.2);
});
test('built anchor update passes exact snapshot, preserves failed selection and commits successful reconstruction',async()=>{
 const next={...world,alignments:[{...line,section_profiles:[{start_height_m:0,end_height_m:6},{start_height_m:6,end_height_m:0}]}]};
 let reject=true;
 vi.mocked(isTauri).mockReturnValue(true);vi.mocked(invoke).mockImplementation(async(command)=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'built-height-test'};
  if(command==='world_snapshot')return world;
  if(command==='list_save_slots')return {active_slot_id:null,slots:[]};
  if(command==='update_alignment_anchor_height'){if(reject)throw Error('勾配制限');return next;}
  if(command==='undo_world_edit')throw Error('built reconstruction is final');
  throw Error(command);
 });render(<App/>);await screen.findByText(/built-height-test/);
 fireEvent.click(screen.getByRole('tab',{name:'選択'}));
 fireEvent.click(screen.getByRole('button',{name:'建設済みアンカー 2 の高さを変更'}));
 const toolbar=screen.getByRole('group',{name:'建設済みアンカーの高さ'});
 fireEvent.click(within(toolbar).getByRole('button',{name:LEVEL_LABELS.ground}));
 expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='update_alignment_anchor_height')).toBe(false);
 fireEvent.click(within(toolbar).getByRole('button',{name:LEVEL_LABELS.elevated}));await screen.findByText(/勾配制限/);
 expect(screen.getByRole('group',{name:'建設済みアンカーの高さ'})).toBeTruthy();
 expect(invoke).toHaveBeenCalledWith('update_alignment_anchor_height',expect.objectContaining({alignmentId:'3',anchorIndex:1,level:'elevated',expectedAlignment:line,constructionPlans:expect.any(Object)}));
 reject=false;fireEvent.click(within(toolbar).getByRole('button',{name:LEVEL_LABELS.elevated}));await waitFor(()=>expect(screen.queryByRole('group',{name:'建設済みアンカーの高さ'})).toBeNull());
 fireEvent.click(screen.getByRole('button',{name:'建設済みアンカー 2 の高さを変更'}));
 const calls=vi.mocked(invoke).mock.calls.filter(([command])=>command==='update_alignment_anchor_height').length;
 fireEvent.click(within(screen.getByRole('group',{name:'建設済みアンカーの高さ'})).getByRole('button',{name:LEVEL_LABELS.elevated}));
 expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==='update_alignment_anchor_height')).toHaveLength(calls);
 // Even if a server returns an unchanged world for a different requested level,
 // retain the committed world and current selection; no new history is invented.
 fireEvent.click(within(screen.getByRole('group',{name:'建設済みアンカーの高さ'})).getByRole('button',{name:LEVEL_LABELS.ground}));
 await waitFor(()=>expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==='update_alignment_anchor_height')).toHaveLength(calls+1));await act(async()=>{});
 expect(screen.getByRole('group',{name:'建設済みアンカーの高さ'})).toBeTruthy();
 await act(async()=>{fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});});expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='undo_world_edit'||command==='undo_alignment_deletion')).toBe(false);
 expect(screen.getByText('建設済み · 高さ 6 m')).toBeTruthy();
});

test('late snapshot cleanup retains a newly selected current anchor, not stale IDs',()=>{
 const oldLine={...line};const newLine={...line,section_profiles:[{start_height_m:0,end_height_m:6},{start_height_m:6,end_height_m:0}]};
 const stale={alignment:oldLine,anchorIndex:1};const selected={alignment:newLine,anchorIndex:1};
 expect(currentBuiltHeightSelection(stale,[newLine],true)).toBeNull();
 expect(currentBuiltHeightSelection(selected,[newLine],true)).toBe(selected);
 expect(currentBuiltHeightSelection(selected,[newLine],false)).toBeNull();
 expect(currentBuiltHeightSelection(selected,[],true)).toBeNull();
});
