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

test('saved path editing explicitly replaces endpoints and retains the last candidate on failure',async()=>{
 let pick:((id:string)=>void)|undefined;
 const onSave=vi.fn(),onCandidateChange=vi.fn();
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  const a=args as {start:string;end:string;viaStationIds?:string[];segments:Candidate['segments']};
  if(command==='preview_route_candidates'){const ids=[a.start,...(a.viaStationIds??[]),a.end];return {status:'unique',candidates:[{stations:ids,segments:ids.slice(1).map((to,i)=>({kind:'alignment',alignment_id:'a',from:ids[i],to}))}],shortest_candidate_index:0,truncated:false};}
  return {path:{stations:[a.start,...a.segments.map(s=>s.to)],segments:a.segments},member_stations:[a.start,...a.segments.map(s=>s.to)],excluded_stations:[]};
 });
 render(<RouteCandidatePreview editRoute={{id:'r',name:'本線',path}} candidateLength={p=>p.stations.at(-1)==='4'?1:10} stations={[...stations,{id:'4',name:'延伸先'}]} worldVersion={world} enabled canApply onSave={onSave} onCandidateChange={onCandidateChange} startSequenceKey={1} mapPickTarget="sequence" onMapPick={(_,cb)=>{pick=cb;}}/>);
 await act(async()=>pick?.('1'));
 expect(invoke).not.toHaveBeenCalledWith('preview_route_candidates',expect.anything());
 await act(async()=>pick?.('2'));await waitFor(()=>expect(onCandidateChange.mock.lastCall?.[0]?.stations).toEqual(['2','3']));
 await act(async()=>pick?.('3'));await act(async()=>pick?.('4'));
 await waitFor(()=>expect(onCandidateChange.mock.lastCall?.[0]?.stations).toEqual(['2','4']));
 await act(async()=>pick?.('3'));await waitFor(()=>expect(onCandidateChange.mock.lastCall?.[0]?.stations).toEqual(['2','3','4']));
 expect(onSave).not.toHaveBeenCalled();
 vi.mocked(invoke).mockResolvedValue({status:'no_path',candidates:[],truncated:false});
 await act(async()=>pick?.('2'));await act(async()=>pick?.('1'));await screen.findByText(/この駅には接続できません/);
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

test.each([false,true])('queued map edits preserve click order; cancelled=%s ignores late results',async cancel=>{
 let pick:((id:string)=>void)|undefined,release!:()=>void;
 const gate=new Promise<void>(resolve=>{release=resolve;});let first=true;
 const onCandidateChange=vi.fn(),onSelectionChange=vi.fn(),onSave=vi.fn();
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  const a=args as {start:string;end:string;viaStationIds?:string[];segments:Candidate['segments']};
  if(command==='preview_route_candidates'){
   if(first){first=false;await gate;}
   const ids=[a.start,...(a.viaStationIds??[]),a.end];
   return {status:'unique',truncated:false,shortest_candidate_index:0,candidates:[{stations:ids,segments:ids.slice(1).map((to,i)=>({kind:'alignment',alignment_id:'a',from:ids[i],to}))}]};
  }
  const ids=[a.start,...a.segments.map(s=>s.to)];return {path:{stations:ids,segments:a.segments},member_stations:ids,excluded_stations:[]};
 });
 render(<RouteCandidatePreview editRoute={{id:'r',name:'本線',path}} stations={[...stations,{id:'4',name:'追加4'},{id:'5',name:'追加5'}]} worldVersion={world} enabled canApply onSave={onSave} onCandidateChange={onCandidateChange} onSelectionChange={onSelectionChange} candidateLength={p=>p.stations.join(',')==='1,2,4,5,3'?1:p.stations.join(',')==='1,2,4,3'?2:10} startSequenceKey={1} onMapPick={(target,callback)=>{pick=target?callback:undefined;}} mapPickTarget="sequence"/>);
 await act(async()=>{pick?.('4');pick?.('5');});
 expect(onSelectionChange).toHaveBeenLastCalledWith(['1','2','3']);
 if(cancel)fireEvent.click(screen.getByText('キャンセル'));
 await act(async()=>release());
 if(cancel){await waitFor(()=>expect(onCandidateChange).toHaveBeenLastCalledWith(null));expect(onSelectionChange).toHaveBeenLastCalledWith(['1','2','3']);}
 else {
  await waitFor(()=>expect(onSelectionChange).toHaveBeenLastCalledWith(['1','2','4','5','3']));
  expect(invoke).toHaveBeenCalledWith('preview_route_candidates',{start:'1',end:'3',viaStationIds:['2','4','5']});
 }
 expect(onSave).not.toHaveBeenCalled();
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
 await act(async()=>pick?.('3'));await act(async()=>pick?.('4'));await screen.findByText('所属駅: 始発 → 新駅');
 expect(invoke).toHaveBeenCalledWith('preview_route_membership',expect.objectContaining({excludedStations:['2']}));
 await act(async()=>pick?.('1'));await act(async()=>pick?.('2'));await screen.findByText('所属駅: 中間 → 新駅');
 expect(onSave).not.toHaveBeenCalled();await act(async()=>fireEvent.click(screen.getByText('編集終了')));
 expect(onSave.mock.calls[0][1].excluded_stations).toEqual([]);
});

test('rejected saved-route endpoint keeps the chosen side active for a direct retry', async()=>{
 let pick:((id:string)=>void)|undefined;
 const onMapPick=vi.fn((_target:unknown,cb?:((id:string)=>void))=>{pick=cb;});
 const edited:Candidate={stations:['2','3'],segments:[{kind:'alignment',alignment_id:'a',from:'2',to:'3'}]};
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command==='preview_route_membership')return {path:edited,member_stations:edited.stations,excluded_stations:[]};
  const a=args as {start:string};
  return a.start==='2'?{status:'unique',candidates:[edited],truncated:false}:{status:'no_path',candidates:[],truncated:false};
 });
 const onSave=vi.fn().mockResolvedValue(undefined);
 render(<RouteCandidatePreview editRoute={{id:'r',name:'本線',path}} stations={[...stations,{id:'4',name:'別線'}]} worldVersion={world} enabled canApply onSave={onSave} startSequenceKey={1} onMapPick={onMapPick}/>);
 await act(async()=>pick?.('1')); // choose the existing start on the map
 await act(async()=>pick?.('4')); // disconnected
 expect((await screen.findByRole('alert')).textContent).toMatch(/元の編集内容を保持/);
 expect(screen.getByText('所属駅: 始発 → 中間 → 終点')).toBeTruthy();
 expect(onMapPick.mock.lastCall?.[0]).toBe('start');
 await act(async()=>pick?.('2')); // retry without clicking start again
 expect(invoke).toHaveBeenLastCalledWith('preview_route_membership',{start:'2',segments:edited.segments,excludedStations:[]});
 expect(screen.getByText('所属駅: 中間 → 終点')).toBeTruthy();
 expect(onMapPick.mock.lastCall?.[0]).toBe('sequence');
 expect(onSave).not.toHaveBeenCalled();
});

test('cancelled endpoint mode ignores a late connected candidate and does not rearm station picks',async()=>{
 let pick:((id:string)=>void)|undefined;
 let resolve!:(r:Candidates)=>void;
 vi.mocked(invoke).mockImplementationOnce(()=>new Promise(r=>{resolve=r;}));
 const onMapPick=vi.fn((_target:unknown,cb?:((id:string)=>void))=>{pick=cb;});
 const onCandidateChange=vi.fn();
 const props={editRoute:{id:'r',name:'本線',path},stations,worldVersion:world,enabled:true,canApply:true,
  startSequenceKey:1,onMapPick,onCandidateChange};
 const h=render(<RouteCandidatePreview {...props} mapPickTarget="sequence"/>);
 await act(async()=>pick?.('3'));
 h.rerender(<RouteCandidatePreview {...props} mapPickTarget="end"/>);
 await act(async()=>pick?.('2'));
 expect(invoke).toHaveBeenCalledWith('preview_route_candidates',{start:'1',end:'2'});
 h.rerender(<RouteCandidatePreview {...props} mapPickTarget={null}/>); // parent Escape or context change
 const calls=onMapPick.mock.calls.length;
 await act(async()=>resolve(unique));
 expect(invoke).toHaveBeenCalledTimes(1);
 expect(onMapPick.mock.calls).toHaveLength(calls);
 expect(onCandidateChange).toHaveBeenLastCalledWith(path);
 expect(screen.queryByText('経路を確認中…')).toBeNull();
});

test('via order uses the native physical path and preserves the previous candidate when a reorder cannot connect',async()=>{
 const ids=['1','2','4','3'];
 const rows=[...stations,{id:'4',name:'経由先'}];
 const build=(order:string[]):Candidate=>({stations:order,segments:order.slice(1).map((to,i)=>({kind:'alignment',alignment_id:'a',from:order[i],to}))});
 const original=build(ids),onSave=vi.fn().mockResolvedValue(undefined);
 let reject=true;
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command==='preview_route_candidates'){
   const a=args as {start:string;end:string;viaStationIds:string[]};
   if(reject)return {status:'no_path',candidates:[],truncated:false};
   return {status:'unique',candidates:[build([a.start,...a.viaStationIds,a.end])],truncated:false};
  }
  const a=args as {start:string;segments:Candidate['segments']};
  const candidate=build([a.start,...a.segments.map(x=>x.to)]);
  return {path:candidate,member_stations:candidate.stations,excluded_stations:[]};
 });
 render(<RouteCandidatePreview entityLabel="系統" editRoute={{id:'service',name:'普通',path:original}} stations={rows} worldVersion={world}
  startSequenceKey={1} mapPickTarget="sequence" enabled canApply onSave={onSave}/>);
 const down=screen.getByRole('button',{name:'中間を一つ後ろへ'});
 fireEvent.click(down);await screen.findByText(/この順序では接続できません/);
 expect(invoke).toHaveBeenCalledWith('preview_route_candidates',{start:'1',end:'3',viaStationIds:['4','2']});
 const order=()=>Array.from(screen.getByRole('list',{name:'選択した駅'}).querySelectorAll('li')).map(li=>li.textContent);
 expect(order()[1]).toContain('中間');
 expect(finish().disabled).toBe(false);
 expect(onSave).not.toHaveBeenCalled();
 reject=false;fireEvent.click(down);
 await waitFor(()=>expect(order()[1]).toContain('経由先'));
 expect(order()[2]).toContain('中間');
 expect(onSave).not.toHaveBeenCalled();
 fireEvent.click(finish());await waitFor(()=>expect(onSave).toHaveBeenCalledWith('普通',expect.objectContaining({path:build(['1','4','2','3'])})));
});

test('station name search uses the same ordered preview and requires explicit apply',async()=>{
 const h=setup();
 const query=screen.getByRole('searchbox',{name:'駅名で検索'});
 for(const name of ['始発','中間','終点']){
  fireEvent.change(query,{target:{value:name}});
  fireEvent.click(screen.getByRole('button',{name}));
 }
 await screen.findByText('所属駅: 始発 → 中間 → 終点');
 expect(invoke).toHaveBeenCalledWith('preview_route_candidates',{start:'1',end:'3',viaStationIds:['2']});
 expect(h.onSave).not.toHaveBeenCalled();
 fireEvent.click(finish());await waitFor(()=>expect(h.onSave).toHaveBeenCalledOnce());
});

test('search normalizes width and case, distinguishes equal names, and caps visible entries',()=>{
 const rows=[{id:'prefix',name:'東ＡBC'},{id:'a',name:'ＡＢＣ'},{id:'b',name:'abc'},
  ...Array.from({length:30},(_,i)=>({id:String(i),name:'abc駅 '+i}))];
 render(<RouteCandidatePreview stations={rows} worldVersion={world} mapPickTarget="sequence" enabled canApply/>);
 fireEvent.change(screen.getByRole('searchbox',{name:'駅名で検索'}),{target:{value:' AbC '}});
 const list=screen.getByRole('list',{name:'駅名検索結果'});
 const labels=Array.from(list.querySelectorAll('button'),x=>x.textContent);
 expect(labels).toHaveLength(24);expect(labels.slice(0,3)).toEqual(['ＡＢＣ 候補 1','abc 候補 2','東ＡBC']);
 expect(screen.getByText('33件（先頭24件を表示）')).toBeTruthy();
});

test('same-name results use their route context and map preview without exposing internal IDs',async()=>{
 const sakuragawa=[
  {id:'s-hanshin-private',name:'桜川'},{id:'s-metro-private',name:'桜川'},{id:'s-omi-private',name:'桜川'},
 ];
 const h=setup();
 const onLocateStation=vi.fn();
 const stationSearchDescription=new Map([
  [sakuragawa[0].id,'阪神なんば線'],[sakuragawa[1].id,'千日前線'],[sakuragawa[2].id,'近江鉄道'],
 ]);
 h.rerender(<RouteCandidatePreview {...h.props} stations={[...stations,...sakuragawa]} stationSearchDescription={stationSearchDescription} onLocateStation={onLocateStation}/>);
 fireEvent.change(screen.getByRole('searchbox',{name:'駅名で検索'}),{target:{value:'桜川'}});
 const box=screen.getByRole('list',{name:'駅名検索結果'});
 expect(box.textContent).not.toMatch(/駅ID|private/);
 const locate=screen.getByRole('button',{name:'桜川 近江鉄道を地図で確認'});
 locate.focus();expect(document.activeElement).toBe(locate);fireEvent.click(locate);
 expect(onLocateStation).toHaveBeenCalledWith('s-omi-private');expect(invoke).not.toHaveBeenCalled();
 const result=screen.getByRole('button',{name:'桜川 千日前線'});
 result.focus();expect(document.activeElement).toBe(result);fireEvent.click(result);
 await h.click('3');
 expect(invoke).toHaveBeenCalledWith('preview_route_candidates',{start:'s-metro-private',end:'3'});
 expect(h.onSave).not.toHaveBeenCalled();
});
