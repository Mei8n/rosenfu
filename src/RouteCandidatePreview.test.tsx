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

test('saved path editing removes a clicked endpoint and inserts new stations without saving',async()=>{
 let pick:((id:string)=>void)|undefined;
 const onSave=vi.fn(),onCandidateChange=vi.fn();
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  const a=args as {start:string;end:string;viaStationIds?:string[];segments:Candidate['segments']};
  if(command==='preview_route_candidates'){const ids=[a.start,...(a.viaStationIds??[]),a.end];return {status:'unique',candidates:[{stations:ids,segments:ids.slice(1).map((to,i)=>({kind:'alignment',alignment_id:'a',from:ids[i],to}))}],shortest_candidate_index:0,truncated:false};}
  return {path:{stations:[a.start,...a.segments.map(s=>s.to)],segments:a.segments},member_stations:[a.start,...a.segments.map(s=>s.to)],excluded_stations:[]};
 });
 render(<RouteCandidatePreview editRoute={{id:'r',name:'本線',path}} candidateLength={p=>p.stations.at(-1)==='4'?1:10} stations={[...stations,{id:'4',name:'延伸先'}]} worldVersion={world} enabled canApply onSave={onSave} onCandidateChange={onCandidateChange} startSequenceKey={1} mapPickTarget="sequence" onMapPick={(_,cb)=>{pick=cb;}}/>);
 await act(async()=>pick?.('1'));await waitFor(()=>expect(onCandidateChange.mock.lastCall?.[0]?.stations).toEqual(['2','3']));
 await act(async()=>pick?.('4'));await waitFor(()=>expect(onCandidateChange.mock.lastCall?.[0]?.stations).toEqual(['2','3','4']));
 expect(onSave).not.toHaveBeenCalled();
 vi.mocked(invoke).mockResolvedValue({status:'no_path',candidates:[],truncated:false});
 await act(async()=>pick?.('2'));await screen.findByText(/この駅には接続できません/);
 expect(onCandidateChange.mock.lastCall?.[0]?.stations).toEqual(['2','3','4']);
});
test('selected station sequence is published for map markers and cleared on reset/unmount',async()=>{
 let pick:((id:string)=>void)|undefined;
 const onSelectionChange=vi.fn();
 const onMapPick=vi.fn((_target:unknown,callback?:((id:string)=>void))=>{pick=callback;});
 const props={stations,worldVersion:world,enabled:true,canApply:true,onMapPick,onSelectionChange,startSequenceKey:1};
 const h=render(<RouteCandidatePreview {...props} resetKey={1}/>);
 expect(onSelectionChange).toHaveBeenLastCalledWith([]);
 await act(async()=>pick?.('1'));
 expect(onSelectionChange).toHaveBeenLastCalledWith(['1']);
 await act(async()=>pick?.('1'));
 expect(onSelectionChange).toHaveBeenLastCalledWith(['1']);
 h.rerender(<RouteCandidatePreview {...props} resetKey={2}/>);
 expect(onSelectionChange).toHaveBeenLastCalledWith([]);
 h.unmount();
 expect(onSelectionChange).toHaveBeenLastCalledWith([]);
});

test('candidate overlay follows validated choice and clears while exploring, resetting and unmounting',async()=>{
 let pick:((id:string)=>void)|undefined;
 const onCandidateChange=vi.fn();
 vi.mocked(invoke).mockImplementation(async(command)=>command==='preview_route_candidates'?unique:{path,member_stations:path.stations,excluded_stations:[]});
 const props={stations,worldVersion:world,enabled:true,canApply:true,onCandidateChange,startSequenceKey:1,onMapPick:(_target:unknown,cb?:((id:string)=>void))=>{pick=cb;}};
 const h=render(<RouteCandidatePreview {...props} resetKey={1}/>);
 expect(onCandidateChange).toHaveBeenLastCalledWith(null);
 await act(async()=>pick?.('1'));
 await act(async()=>pick?.('3'));
 expect(onCandidateChange).toHaveBeenLastCalledWith(path);
 let finishSearch!:(value:Candidates)=>void;
 vi.mocked(invoke).mockImplementationOnce(()=>new Promise(r=>{finishSearch=r;}));
 await act(async()=>pick?.('2'));
 expect(onCandidateChange).toHaveBeenLastCalledWith(null);
 await act(async()=>finishSearch(unique));
 expect(onCandidateChange).toHaveBeenLastCalledWith(path);
 h.rerender(<RouteCandidatePreview {...props} resetKey={2}/>);
 expect(onCandidateChange).toHaveBeenLastCalledWith(null);
 h.unmount();
 expect(onCandidateChange).toHaveBeenLastCalledWith(null);
});
test('switching alternative candidate updates map overlay only after validation',async()=>{
 const alternate:Candidate={stations:['1','2','3'],segments:[{kind:'alignment',alignment_id:'other',from:'1',to:'3'}]};
 const response:Candidates={status:'ambiguous',candidates:[path,alternate],truncated:false,shortest_candidate_index:0};
 let pick:((id:string)=>void)|undefined;
 const onCandidateChange=vi.fn();
 vi.mocked(invoke).mockImplementation(async(command,args)=>command==='preview_route_candidates'?response:{path:response.candidates.find(c=>JSON.stringify(c.segments)===JSON.stringify((args as {segments:unknown}).segments)),member_stations:path.stations,excluded_stations:[]});
 render(<RouteCandidatePreview stations={stations} worldVersion={world} enabled canApply onCandidateChange={onCandidateChange} startSequenceKey={1} onMapPick={(_,cb)=>{pick=cb;}}/>);
 await act(async()=>pick?.('1'));await act(async()=>pick?.('3'));
 expect(onCandidateChange).toHaveBeenLastCalledWith(path);
 const choices=screen.getAllByRole('radio');
 await act(async()=>fireEvent.click(choices[1]));
 expect(onCandidateChange).toHaveBeenLastCalledWith(alternate);
});
test('after world revalidation map clicks use fresh station membership and distance function',async()=>{
 let pick:((id:string)=>void)|undefined;
 const oldLength=vi.fn(()=>1),newLength=vi.fn(()=>2);
 vi.mocked(invoke).mockImplementation(async command=>command==='preview_route_candidates'?unique:{path,member_stations:path.stations,excluded_stations:[]});
 const props={editRoute:{id:'r',name:'本線',path},stations,worldVersion:world,enabled:true,canApply:true,startSequenceKey:1,onMapPick:(_t:unknown,cb?:((id:string)=>void))=>{pick=cb;}};
 const h=render(<RouteCandidatePreview {...props} candidateLength={oldLength}/>);
 h.rerender(<RouteCandidatePreview {...props} stations={[...stations,{id:'4',name:'新駅'}]} worldVersion={{}} candidateLength={newLength}/>);
 await act(async()=>{fireEvent.click(screen.getByText('経路を再確認'));});
 await act(async()=>pick?.('4'));
 await waitFor(()=>expect(newLength).toHaveBeenCalled());expect(oldLength).not.toHaveBeenCalled();
});
test('rename-only and cancellation preserve saved intermediate membership exclusion',async()=>{
 const onSave=vi.fn().mockResolvedValue(undefined),onCancelEdit=vi.fn();
 const props={editRoute:{id:'r',name:'本線',path,memberStations:['1','3'],excludedStations:['2']},stations,worldVersion:world,enabled:true,canApply:true,startSequenceKey:1,onSave,onCancelEdit};
 render(<RouteCandidatePreview {...props}/>);
 expect(screen.getByText('所属駅: 始発 → 終点')).toBeTruthy();
 fireEvent.change(screen.getByLabelText('路線名'),{target:{value:'新本線'}});
 expect(onSave).not.toHaveBeenCalled();
 await act(async()=>fireEvent.click(screen.getByText('編集終了')));
 expect(onSave).toHaveBeenCalledWith('新本線',{path,member_stations:['1','3'],excluded_stations:['2']});
 fireEvent.click(screen.getByText('キャンセル'));expect(onSave).toHaveBeenCalledOnce();expect(onCancelEdit).toHaveBeenCalledOnce();
});
test('changed route retains excluded intermediate stations but promotes a new endpoint to membership',async()=>{
 let pick:((id:string)=>void)|undefined;const onSave=vi.fn();
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  const a=args as {start:string;end:string;viaStationIds?:string[];segments:Candidate['segments'];excludedStations?:string[]};
  if(command==='preview_route_candidates'){const ids=[a.start,...(a.viaStationIds??[]),a.end];return {status:'unique',candidates:[{stations:ids,segments:ids.slice(1).map((to,i)=>({kind:'alignment',alignment_id:'a',from:ids[i],to}))}],truncated:false,shortest_candidate_index:0};}
  const ids=[a.start,...a.segments.map(s=>s.to)];return {path:{stations:ids,segments:a.segments},member_stations:ids.filter(id=>!a.excludedStations?.includes(id)),excluded_stations:a.excludedStations??[]};
 });
 render(<RouteCandidatePreview editRoute={{id:'r',name:'本線',path,memberStations:['1','3'],excludedStations:['2']}} candidateLength={p=>p.stations.at(-1)==='4'?1:10} stations={[...stations,{id:'4',name:'新駅'}]} worldVersion={world} enabled canApply startSequenceKey={1} onSave={onSave} onMapPick={(_,cb)=>{pick=cb;}}/>);
 await act(async()=>pick?.('4'));await screen.findByText('所属駅: 始発 → 終点 → 新駅');
 expect(invoke).toHaveBeenCalledWith('preview_route_membership',expect.objectContaining({excludedStations:['2']}));
 await act(async()=>pick?.('1'));await screen.findByText('所属駅: 中間 → 終点 → 新駅');
 expect(onSave).not.toHaveBeenCalled();await act(async()=>fireEvent.click(screen.getByText('編集終了')));
 expect(onSave.mock.calls[0][1].excluded_stations).toEqual([]);
});
