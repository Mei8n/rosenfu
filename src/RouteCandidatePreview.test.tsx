// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi,type Mock} from 'vitest';
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {RouteCandidatePreview,type Candidate,type Candidates,type Membership} from './RouteCandidatePreview';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
const stations=[{id:'1',name:'始発'},{id:'2',name:'中間'},{id:'3',name:'終点'}];
const path:Candidate={stations:['1','2','3'],segments:[{kind:'alignment',alignment_id:'a',from:'1',to:'3'}]};
const unique:Candidates={status:'unique',candidates:[path],truncated:false,shortest_candidate_index:0};
const world={};
function setup(response:Candidates=unique,options:{canApply?:boolean;onSave?:Mock<(name:string,membership:Membership)=>Promise<void>>}={}){
 let pick:((id:string)=>void)|undefined;
 const onSave=options.onSave??vi.fn().mockResolvedValue(undefined),onCancelEdit=vi.fn();
 const onMapPick=vi.fn((target:unknown,callback?:((id:string)=>void))=>{pick=target?callback:undefined;});
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command==='preview_route_candidates')return response;
  if(command==='preview_route_membership'){const candidate=response.candidates.find(c=>JSON.stringify(c.segments)===JSON.stringify((args as {segments:unknown}).segments))!;return {path:candidate,member_stations:candidate.stations,excluded_stations:[]};}
  throw Error(command);
 });
 const props={stations,worldVersion:world,enabled:true,canApply:options.canApply??true,onSave,onCancelEdit,onMapPick,startSequenceKey:1,mapPickTarget:'sequence' as const};
 const view=render(<RouteCandidatePreview {...props}/>);
 const click=async(id:string)=>{await act(async()=>{pick?.(id);});};
 return {...view,props,click,onSave,onMapPick,onCancelEdit};
}
const finish=()=>screen.getByRole('button',{name:'編集終了'}) as HTMLButtonElement;
test('sequential selection automatically previews all physical members and only Finish persists',async()=>{
 const h=setup();expect((screen.getByLabelText('路線名') as HTMLInputElement).value).toBe('路線1');
 await h.click('1');expect(invoke).not.toHaveBeenCalled();expect(finish().disabled).toBe(true);
 await h.click('3');await screen.findByText('所属駅: 始発 → 中間 → 終点');
 expect(invoke).toHaveBeenCalledWith('preview_route_membership',{start:'1',segments:path.segments,excludedStations:[]});
 expect(h.onSave).not.toHaveBeenCalled();fireEvent.click(finish());
 await waitFor(()=>expect(h.onSave).toHaveBeenCalledWith('路線1',{path,member_stations:path.stations,excluded_stations:[]}));
});
test('failed finish retains name, selected stations and candidate for retry; cancellation never saves',async()=>{
 const h=setup(unique,{onSave:vi.fn().mockRejectedValueOnce(Error('系統が使用中')).mockResolvedValue(undefined)});
 await h.click('1');await h.click('3');fireEvent.change(screen.getByLabelText('路線名'),{target:{value:'変更名'}});fireEvent.click(finish());
 await screen.findByText('Error: 系統が使用中');expect((screen.getByLabelText('路線名') as HTMLInputElement).value).toBe('変更名');
 expect(screen.getByText('所属駅: 始発 → 中間 → 終点')).toBeTruthy();
 fireEvent.click(finish());await waitFor(()=>expect(h.onSave).toHaveBeenCalledTimes(2));
 fireEvent.click(screen.getByRole('button',{name:'キャンセル'}));expect(h.onSave).toHaveBeenCalledTimes(2);expect(h.onCancelEdit).toHaveBeenCalledOnce();
});
test('selected order becomes constrained via list and duplicates never create extra searches',async()=>{
 const h=setup();await h.click('1');await h.click('2');await h.click('3');
 expect(invoke).toHaveBeenCalledWith('preview_route_candidates',{start:'1',end:'3',viaStationIds:['2']});
 const count=vi.mocked(invoke).mock.calls.length;await h.click('2');expect(screen.getByRole('alert').textContent).toContain('重複');expect(vi.mocked(invoke).mock.calls).toHaveLength(count);
});
test('new station selections discard late earlier search results',async()=>{
 const h=setup();let resolve!:(r:Candidates)=>void;
 vi.mocked(invoke).mockImplementationOnce(()=>new Promise(r=>{resolve=r;}));await h.click('1');await h.click('2');await h.click('3');
 await screen.findByText('所属駅: 始発 → 中間 → 終点');await act(async()=>resolve({status:'no_path',candidates:[],truncated:false}));
 expect(screen.queryByText('接続する経路がありません。')).toBeNull();expect(finish().disabled).toBe(false);
});
test('world replacement invalidates membership and requires a new search before finishing',async()=>{
 const h=setup();await h.click('1');await h.click('3');h.rerender(<RouteCandidatePreview {...h.props} worldVersion={{}}/>);
 expect(finish().disabled).toBe(true);fireEvent.click(screen.getByRole('button',{name:'経路を再確認'}));
 await waitFor(()=>expect(finish().disabled).toBe(false));expect(h.onSave).not.toHaveBeenCalled();
});
test('no path, failed lookup and retry stay distinct and cannot save',async()=>{
 const h=setup({status:'no_path',candidates:[],truncated:false});await h.click('1');await h.click('2');
 expect(screen.getByText('接続する経路がありません。')).toBeTruthy();expect(finish().disabled).toBe(true);
 vi.mocked(invoke).mockRejectedValueOnce(Error('lookup failed'));await h.click('3');expect(screen.getByRole('alert').textContent).toContain('lookup failed');expect(h.onSave).not.toHaveBeenCalled();
});
test('incomplete candidate sets never select or validate an alleged shortest result',async()=>{
 const h=setup({...unique,status:'incomplete',truncated:true});await h.click('1');await h.click('3');
 expect(screen.getByText(/探索上限/)).toBeTruthy();expect(finish().disabled).toBe(true);
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='preview_route_membership')).toBe(false);
});
test('busy save is deduplicated and unapplied competing changes block Finish',async()=>{
 let done!:()=>void;const onSave=vi.fn(()=>new Promise<void>(r=>{done=r;}));const h=setup(unique,{onSave});await h.click('1');await h.click('3');
 fireEvent.click(finish());fireEvent.click(finish());expect(onSave).toHaveBeenCalledOnce();expect((screen.getByLabelText('路線名') as HTMLInputElement).disabled).toBe(true);
 await act(async()=>done());h.rerender(<RouteCandidatePreview {...h.props} canApply={false}/>);expect(finish().disabled).toBe(true);
});
test('world change and cancellation discard late membership responses without making a save possible',async()=>{
 const h=setup();let resolve!:(v:unknown)=>void;
 vi.mocked(invoke).mockResolvedValueOnce(unique).mockImplementationOnce(()=>new Promise(r=>{resolve=r;}));
 await h.click('1');await h.click('3');h.rerender(<RouteCandidatePreview {...h.props} worldVersion={{}}/>);
 await act(async()=>resolve({path,member_stations:path.stations,excluded_stations:[]}));
 expect(screen.queryByText('所属駅: 始発 → 中間 → 終点')).toBeNull();expect(finish().disabled).toBe(true);
 let late!:(v:unknown)=>void;vi.mocked(invoke).mockResolvedValueOnce(unique).mockImplementationOnce(()=>new Promise(r=>{late=r;}));
 fireEvent.click(screen.getByRole('button',{name:'経路を再確認'}));await waitFor(()=>expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==='preview_route_membership')).toHaveLength(2));
 fireEvent.click(screen.getByRole('button',{name:'キャンセル'}));await act(async()=>late({path,member_stations:path.stations,excluded_stations:[]}));
 expect(screen.queryByText('所属駅: 始発 → 中間 → 終点')).toBeNull();expect(h.onSave).not.toHaveBeenCalled();
});

test('alternative window defaults to the certified shortest and can return to map without saving',async()=>{
 const other:Candidate={stations:['1','3'],segments:[{kind:'alignment',alignment_id:'b',from:'1',to:'3'}]};
 const h=setup({status:'ambiguous',candidates:[path,other],shortest_candidate_index:1,truncated:false});
 await h.click('1');await h.click('3');
 expect(screen.getByRole('dialog',{name:'路線の経路を選択'})).toBeTruthy();
 expect((screen.getAllByRole('radio')[1] as HTMLInputElement).checked).toBe(true);
 vi.mocked(invoke).mockRejectedValueOnce(Error('候補を検証できません'));
 fireEvent.click(screen.getAllByRole('radio')[0]);
 await waitFor(()=>expect(finish().disabled).toBe(true));
 fireEvent.click(screen.getByRole('button',{name:'地図で選び直す'}));
 expect(screen.queryByRole('dialog')).toBeNull();expect(h.onSave).not.toHaveBeenCalled();
 await h.click('2');expect(invoke).toHaveBeenCalledWith('preview_route_candidates',{start:'1',end:'2',viaStationIds:['3']});
});
