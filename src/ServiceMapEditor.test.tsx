// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {ServiceMapEditor} from './ServiceMapEditor';
import {resolveServicePath,type ServicePath} from './servicePaths';
import type {Segment} from './RouteCandidatePreview';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
const world={station_placements:['0','1','2','3','4'].map((station_id,i)=>({station_id,alignment_id:'a',position:i/4}))};
const path:ServicePath={service_id:'s',next_leg_key:'9',legs:[{key:'7',segment:{kind:'alignment',alignment_id:'a',from:'1',to:'3'}}],stops:[{kind:'start'},{kind:'on_leg',leg_key:'7',station_id:'3'}]};
function setup(candidateGate?:Promise<void>,applyGate?:Promise<void>){
 let pick:((id:string)=>void)|undefined;const onSave=vi.fn().mockResolvedValue(undefined);
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  const a=args as {start:string;end:string;segments:Segment[];viaStationIds?:string[]};
  if(command==='preview_route_candidates'){await candidateGate;const ids=[a.start,...(a.viaStationIds??[]),a.end];const segments=ids.slice(1).map((to,i)=>({kind:'alignment',alignment_id:'a',from:ids[i],to}));return {status:'unique',candidates:[{stations:ids,segments}],truncated:false,shortest_candidate_index:0};}
  if(command==='preview_route_membership')return {path:{stations:[a.start,...a.segments.map(s=>s.to)],segments:a.segments},member_stations:[a.start,...a.segments.map(s=>s.to)],excluded_stations:[]};
  if(command==='preview_service_path_replacement'){await applyGate;const legs=a.segments.map((segment,i)=>({key:String(9+i),segment}));return {legs,occurrences:resolveServicePath(a.start,legs,world),requires_stop_selection:true};}
  throw Error(command);
 });
 const props={service:{id:'s',name:'急行',start:'1'},path,binding:{kind:'routes' as const,route_ids:['r1','r2']},world,stations:['0','1','2','3','4'].map(id=>({id,name:`駅${id}`})),enabled:true,candidateLength:(p:{stations:string[]})=>p.stations.slice(1).reduce((sum,id,i)=>sum+Math.abs(Number(id)-Number(p.stations[i])),0),onDirty:vi.fn(),onSave,onCandidateChange:vi.fn(),onSelectionChange:vi.fn(),onMapPick:(_t:unknown,cb?:((id:string)=>void))=>{pick=cb;},mapPickTarget:'sequence' as const};
 const view=render(<ServiceMapEditor {...props}/>);fireEvent.click(screen.getByText('地図で系統を編集'));return {...view,props,click:async(id:string)=>{await act(async()=>pick?.(id));}};
}
test('map endpoint extension preserves express passing and through binding until explicit Finish',async()=>{
 const h=setup();await h.click('3');await h.click('4');await waitFor(()=>expect(h.props.onCandidateChange.mock.lastCall?.[0]?.stations).toEqual(['1','2','4']));
 expect(h.props.onSave).not.toHaveBeenCalled();fireEvent.click(screen.getByText('編集終了'));await waitFor(()=>expect(h.props.onSave).toHaveBeenCalledOnce());
 const saved=h.props.onSave.mock.calls[0][0];expect(saved.sourceRouteIds).toEqual(['r1','r2']);expect(saved.name).toBe('急行');expect(saved.stops).toEqual([{kind:'start'},{kind:'on_leg',leg_key:'10',station_id:'3'},{kind:'on_leg',leg_key:'10',station_id:'4'}]);
});
test('cancel preserves persisted service and stale world blocks application',async()=>{
 const h=setup();await h.click('3');await h.click('4');h.rerender(<ServiceMapEditor {...h.props} world={{...world}}/>);
 expect((screen.getByText('編集終了') as HTMLButtonElement).disabled).toBe(true);fireEvent.click(screen.getByText('キャンセル'));expect(h.props.onSave).not.toHaveBeenCalled();expect(screen.getByText('地図で系統を編集')).toBeTruthy();
});
test('clicking an existing middle pass station makes it stop without rerouting',async()=>{
 const h=setup();await h.click('2');expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='preview_route_candidates')).toBe(false);
 fireEvent.click(screen.getByText('編集終了'));await waitFor(()=>expect(h.props.onSave).toHaveBeenCalledOnce());
 expect(h.props.onSave.mock.calls[0][0].stops).toEqual([{kind:'start'},{kind:'on_leg',leg_key:'9',station_id:'2'},{kind:'on_leg',leg_key:'9',station_id:'3'}]);
});

test('map start extension keeps existing pass settings and binding until explicit Finish',async()=>{
 const h=setup();await h.click('1');await h.click('0');await waitFor(()=>expect(h.props.onCandidateChange.mock.lastCall?.[0]?.stations).toEqual(['0','2','3']));
 expect(h.props.onSave).not.toHaveBeenCalled();fireEvent.click(screen.getByText('編集終了'));await waitFor(()=>expect(h.props.onSave).toHaveBeenCalledOnce());
 expect(h.props.onSave.mock.calls[0][0]).toMatchObject({start:'0',sourceRouteIds:['r1','r2'],stops:[{kind:'start'},{kind:'on_leg',leg_key:'9',station_id:'1'},{kind:'on_leg',leg_key:'10',station_id:'3'}]});
});
test.each([['1','2',['2','3']],['3','1',['1','2']]])('removing endpoint %s shortens draft and preserves mandatory endpoints',async(id,start,ids)=>{
 const h=setup();await h.click(id as string);await h.click('2');await waitFor(()=>expect(h.props.onCandidateChange.mock.lastCall?.[0]?.stations).toEqual(ids));
 expect(h.props.onSave).not.toHaveBeenCalled();fireEvent.click(screen.getByText('編集終了'));await waitFor(()=>expect(h.props.onSave).toHaveBeenCalledOnce());
 expect(h.props.onSave.mock.calls[0][0]).toMatchObject({start,sourceRouteIds:['r1','r2'],stops:[{kind:'start'},{kind:'on_leg',leg_key:'9',station_id:(ids as string[])[1]}]});
});

test('clicking a new station without a designated endpoint keeps both endpoints',async()=>{
 const h=setup();await h.click('4');
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('preview_route_candidates',expect.objectContaining({start:'1',end:'3'})));
 expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==='preview_route_candidates').every(([,args])=>((args as {start:string;end:string}).start==='1'&&(args as {start:string;end:string}).end==='3'))).toBe(true);
 expect(h.props.onSave).not.toHaveBeenCalled();
});

test('a cancelled endpoint lookup cannot restore the old draft preview',async()=>{
 let resolve!:()=>void;const gate=new Promise<void>(done=>{resolve=done;});
 const h=setup(gate);await h.click('3');await h.click('4');
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('preview_route_candidates',{start:'1',end:'4',viaStationIds:['2']}));
 fireEvent.click(screen.getByText('キャンセル'));
 await act(async()=>resolve());
 expect(h.props.onCandidateChange.mock.lastCall?.[0]).toBeNull();
 expect(h.props.onSelectionChange.mock.lastCall?.[0]).toEqual([]);
 expect(h.props.onSave).not.toHaveBeenCalled();
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='preview_route_membership')).toBe(false);
});

test('disconnected series endpoint can be retried without changing stop overrides',async()=>{
 const h=setup();
 vi.mocked(invoke).mockResolvedValueOnce({status:'no_path',candidates:[],truncated:false});
 await h.click('3');await h.click('4');
 expect(await screen.findByText(/この駅には接続できません/)).toBeTruthy();
 await h.click('2');
 await waitFor(()=>expect(h.props.onCandidateChange.mock.lastCall?.[0]?.stations).toEqual(['1','2']));
 expect(h.props.onSave).not.toHaveBeenCalled();
 fireEvent.click(screen.getByText('編集終了'));await waitFor(()=>expect(h.props.onSave).toHaveBeenCalledOnce());
 expect(h.props.onSave.mock.calls[0][0]).toMatchObject({
  start:'1',sourceRouteIds:['r1','r2'],stops:[{kind:'start'},{kind:'on_leg',leg_key:'9',station_id:'2'}],
 });
});


test('a retired saved-series preflight does not apply after leaving the editor',async()=>{
 let resolve!:()=>void;const gate=new Promise<void>(done=>{resolve=done;});
 const h=setup(undefined,gate);
 fireEvent.click(screen.getByText('編集終了'));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('preview_service_path_replacement',expect.objectContaining({serviceId:'s'})));
 h.unmount();
 await act(async()=>resolve());
 expect(h.props.onSave).not.toHaveBeenCalled();
});

test('disabling a pending saved-series edit retires its preflight but keeps explicit retries',async()=>{
 let resolve!:()=>void;const gate=new Promise<void>(done=>{resolve=done;});
 const h=setup(undefined,gate);
 fireEvent.change(screen.getByRole('textbox',{name:'系統名'}),{target:{value:'急行の変更'}});
 await h.click('2');
 const finish=()=>fireEvent.click(screen.getByText('編集終了'));
 finish();
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('preview_service_path_replacement',expect.objectContaining({serviceId:'s'})));
 // Native validation is not a write. Even when enabled again with identical
 // props, the earlier preflight must not authorize a later apply.
 h.rerender(<ServiceMapEditor {...h.props} enabled={false}/>);
 h.rerender(<ServiceMapEditor {...h.props}/>);
 await act(async()=>resolve());
 expect(h.props.onSave).not.toHaveBeenCalled();
 expect((await screen.findByRole('alert')).textContent).toMatch(/入力は保持/);
 expect(screen.getByRole<HTMLInputElement>('textbox',{name:'系統名'}).value).toBe('急行の変更');
 h.props.onSave.mockRejectedValueOnce(Error('別の編集と競合しました'));
 finish();
 await screen.findByText(/別の編集と競合しました/);
 expect(screen.getByRole<HTMLInputElement>('textbox',{name:'系統名'}).value).toBe('急行の変更');
 finish();
 await waitFor(()=>expect(h.props.onSave).toHaveBeenCalledTimes(2));
 for(const [saved] of h.props.onSave.mock.calls)expect(saved).toMatchObject({
  name:'急行の変更',sourceRouteIds:['r1','r2'],
  stops:[{kind:'start'},{kind:'on_leg',leg_key:'9',station_id:'2'},{kind:'on_leg',leg_key:'9',station_id:'3'}],
 });
 await screen.findByText('地図で系統を編集');
});

function denseStopEditor(){
 const stations=Array.from({length:240},(_,i)=>({id:String(i+1),name:`駅${i+1}`}));
 let stopReads=0,placementReads=0,pick:((id:string)=>void)|undefined;
 function trace<T>(rows:T[],read:()=>void){
  return new Proxy(rows,{get(target,key,receiver){
   if(typeof key==='string'&&/^\d+$/.test(key))read();
   return Reflect.get(target,key,receiver);
  }});
 }
 const world={station_placements:trace(stations.map(({id:station_id},i)=>({station_id,alignment_id:'dense',position:i/239})),()=>placementReads++)};
 const path:ServicePath={service_id:'s',next_leg_key:'9',
  legs:[{key:'7',segment:{kind:'alignment',alignment_id:'dense',from:'1',to:'240'}}],
  stops:trace([{kind:'start'},...stations.slice(1).filter((_,i)=>i%2===1||i===238)
   .map(({id:station_id})=>({kind:'on_leg',leg_key:'7',station_id} as const))],()=>stopReads++)};
 const onSave=vi.fn().mockResolvedValue(undefined);
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command!=='preview_service_path_replacement')throw Error(command);
  const {start,segments}=args as {start:string;segments:Segment[]};
  const legs=segments.map((segment,i)=>({key:String(9+i),segment}));
  return {legs,occurrences:resolveServicePath(start,legs,world),requires_stop_selection:true};
 });
 const {container}=render(<ServiceMapEditor service={{id:'s',name:'長距離急行',start:'1'}} path={path} binding={{kind:'route',route_id:'r1'}}
  world={world} stations={stations} enabled onDirty={vi.fn()} onSave={onSave}
  candidateLength={()=>0} onCandidateChange={vi.fn()} onSelectionChange={vi.fn()}
  onMapPick={(_target,select)=>{pick=select;}} mapPickTarget="sequence"/>);
 fireEvent.click(screen.getByText('地図で系統を編集'));
 return {
  toggle:(...ids:string[])=>act(()=>ids.forEach(id=>pick?.(id))),onSave,
  reads:()=>({stopReads,placementReads}),
  displayed:(id:string)=>{
   // Match the accessible label directly instead of enumerating every
   // label and associated control in the 240-row diagram on each read.
   const controls=container.querySelectorAll<HTMLInputElement>(`input[type="checkbox"][aria-label="地図選択の停車駅 ${id}"]`);
   expect(controls, `expected one stop marker for row ${id}`).toHaveLength(1);
   return controls[0].checked;
  },
 };
}

test('a dense stop-only edit reuses the committed geometry and stop index',()=>{
 const h=denseStopEditor();
 const initial=h.reads();
 expect(h.displayed('122')).toBe(false);
 h.toggle('122');
 expect(h.displayed('122')).toBe(true);
 h.toggle('125');
 expect(h.displayed('125')).toBe(false);
 expect(h.reads()).toEqual(initial); // no rereads of either committed array
 expect(initial).toEqual({stopReads:121,placementReads:240});
 expect(invoke).not.toHaveBeenCalled();
 expect(h.onSave).not.toHaveBeenCalled();
});

test('two fast map clicks restore the original stop toggle and explicit apply keeps it',async()=>{
 const h=denseStopEditor();
 expect(h.displayed('122')).toBe(false);
 h.toggle('122','122');
 expect(h.displayed('122')).toBe(false);
 h.toggle('124');
 expect(h.displayed('124')).toBe(true);
 expect(invoke).not.toHaveBeenCalled();
 fireEvent.click(screen.getByText('編集終了'));
 await waitFor(()=>expect(h.onSave).toHaveBeenCalledOnce());
 const saved=h.onSave.mock.calls[0][0];
 expect(saved.sourceRouteId).toBe('r1');
 expect(saved.stops).not.toContainEqual({kind:'on_leg',leg_key:'9',station_id:'122'});
 expect(saved.stops).toContainEqual({kind:'on_leg',leg_key:'9',station_id:'124'});
 expect(saved.stops).toContainEqual({kind:'on_leg',leg_key:'9',station_id:'240'});
 expect(vi.mocked(invoke).mock.calls.map(([command])=>command)).toEqual(['preview_service_path_replacement']);
});
