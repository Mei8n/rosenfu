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
const world={station_placements:['1','2','3','4'].map((station_id,i)=>({station_id,alignment_id:'a',position:i/3}))};
const path:ServicePath={service_id:'s',next_leg_key:'9',legs:[{key:'7',segment:{kind:'alignment',alignment_id:'a',from:'1',to:'3'}}],stops:[{kind:'start'},{kind:'on_leg',leg_key:'7',station_id:'3'}]};
function setup(){
 let pick:((id:string)=>void)|undefined;const onSave=vi.fn().mockResolvedValue(undefined);
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  const a=args as {start:string;end:string;segments:Segment[];viaStationIds?:string[]};
  if(command==='preview_route_candidates'){const ids=[a.start,...(a.viaStationIds??[]),a.end];const segments=ids.slice(1).map((to,i)=>({kind:'alignment',alignment_id:'a',from:ids[i],to}));return {status:'unique',candidates:[{stations:ids,segments}],truncated:false,shortest_candidate_index:0};}
  if(command==='preview_route_membership')return {path:{stations:[a.start,...a.segments.map(s=>s.to)],segments:a.segments},member_stations:[a.start,...a.segments.map(s=>s.to)],excluded_stations:[]};
  if(command==='preview_service_path_replacement'){const legs=a.segments.map((segment,i)=>({key:String(9+i),segment}));return {legs,occurrences:resolveServicePath(a.start,legs,world),requires_stop_selection:true};}
  throw Error(command);
 });
 const props={service:{id:'s',name:'急行',start:'1'},path,binding:{kind:'routes' as const,route_ids:['r1','r2']},world,stations:['1','2','3','4'].map(id=>({id,name:`駅${id}`})),enabled:true,candidateLength:(p:{stations:string[]})=>p.stations.at(-1)==='4'?1:10,onDirty:vi.fn(),onSave,onCandidateChange:vi.fn(),onSelectionChange:vi.fn(),onMapPick:(_t:unknown,cb?:((id:string)=>void))=>{pick=cb;},mapPickTarget:'sequence' as const};
 const view=render(<ServiceMapEditor {...props}/>);fireEvent.click(screen.getByText('地図で系統を編集'));return {...view,props,click:async(id:string)=>{await act(async()=>pick?.(id));}};
}
test('map endpoint extension preserves express passing and through binding until explicit Finish',async()=>{
 const h=setup();await h.click('4');await waitFor(()=>expect(h.props.onCandidateChange.mock.lastCall?.[0]?.stations).toEqual(['1','2','3','4']));
 expect(h.props.onSave).not.toHaveBeenCalled();fireEvent.click(screen.getByText('編集終了'));await waitFor(()=>expect(h.props.onSave).toHaveBeenCalledOnce());
 const saved=h.props.onSave.mock.calls[0][0];expect(saved.sourceRouteIds).toEqual(['r1','r2']);expect(saved.name).toBe('急行');expect(saved.stops).toEqual([{kind:'start'},{kind:'on_leg',leg_key:'10',station_id:'3'},{kind:'on_leg',leg_key:'11',station_id:'4'}]);
});
test('cancel preserves persisted service and stale world blocks application',async()=>{
 const h=setup();await h.click('4');h.rerender(<ServiceMapEditor {...h.props} world={{...world}}/>);
 expect((screen.getByText('編集終了') as HTMLButtonElement).disabled).toBe(true);fireEvent.click(screen.getByText('キャンセル'));expect(h.props.onSave).not.toHaveBeenCalled();expect(screen.getByText('地図で系統を編集')).toBeTruthy();
});
test('clicking an existing middle pass station makes it stop without rerouting',async()=>{
 const h=setup();await h.click('2');expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='preview_route_candidates')).toBe(false);
 fireEvent.click(screen.getByText('編集終了'));await waitFor(()=>expect(h.props.onSave).toHaveBeenCalledOnce());
 expect(h.props.onSave.mock.calls[0][0].stops).toEqual([{kind:'start'},{kind:'on_leg',leg_key:'9',station_id:'2'},{kind:'on_leg',leg_key:'9',station_id:'3'}]);
});
