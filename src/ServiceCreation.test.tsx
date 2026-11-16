// @vitest-environment jsdom
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {ServiceMapCreator} from './ServiceMapCreator';
import {ServiceFromRoute} from './ServiceFromRoute';
import {resolveServicePath} from './servicePaths';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
const stations=['a','b','c'].map(id=>({id,name:id.toUpperCase()}));
const world={station_placements:stations.map((s,i)=>({station_id:s.id,alignment_id:'track',position:i/2}))};
const segments=[{kind:'alignment' as const,alignment_id:'track',from:'a',to:'c'}];
const legs=[{key:'1',segment:segments[0]}];
const preview={legs,occurrences:resolveServicePath('a',legs,world),requires_stop_selection:true};
const membership={path:{stations:['a','c'],segments},member_stations:['a','b','c'],excluded_stations:[]};
function responses(){vi.mocked(invoke).mockImplementation(async command=>command==='preview_route_candidates'?{status:'unique',candidates:[membership.path],truncated:false,shortest_candidate_index:0}:command==='preview_route_membership'?membership:preview);}
test('map station sequence finds path and creates independent service stopping at every intermediate station',async()=>{
 responses();let select:(id:string)=>void=()=>{};const save=vi.fn().mockResolvedValue(undefined);
 render(<ServiceMapCreator world={world} stations={stations} enabled defaultName="系統1" onDirty={vi.fn()} onSave={save} onCancel={vi.fn()} onMapPick={(_target,handler)=>{if(handler)select=handler;}} mapPickTarget="sequence" startSequenceKey={1}/>);
 await act(async()=>{select('a');});await act(async()=>{select('c');});
 await waitFor(()=>expect((screen.getByRole('button',{name:'編集終了'}) as HTMLButtonElement).disabled).toBe(false));
 fireEvent.click(screen.getByRole('button',{name:'編集終了'}));await waitFor(()=>expect(save).toHaveBeenCalledWith({name:'系統1',start:'a',segments,sourceRouteIds:[],stops:preview.occurrences.map(o=>o.reference)}));
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='save_planned_route')).toBe(false);
});
test('saved route selection automatically previews all stops and preserves explicit binding',async()=>{
 responses();const save=vi.fn().mockResolvedValue(undefined);
 render(<ServiceFromRoute routes={[{id:'r',name:'本線',start:'a'}]} paths={[{route_id:'r',segments}]} stations={stations} world={world} enabled onDirty={vi.fn()} onSave={save}/>);
 fireEvent.change(screen.getByLabelText('系統にする路線'),{target:{value:'r'}});
 await screen.findByLabelText('停車駅 2');expect((screen.getByLabelText('停車駅 2') as HTMLInputElement).checked).toBe(true);
 fireEvent.click(screen.getByRole('button',{name:'系統を作成'}));await waitFor(()=>expect(save).toHaveBeenCalledWith({name:'本線',sourceRouteId:'r',start:'a',segments,stops:preview.occurrences.map(o=>o.reference)}));
});
test('late route response after cancellation cannot restore discarded creation',async()=>{
 let resolve:(value:unknown)=>void=()=>{};vi.mocked(invoke).mockImplementation(()=>new Promise(r=>resolve=r));
 const props={routes:[{id:'r',name:'本線',start:'a'}],paths:[{route_id:'r',segments}],stations,world,enabled:true,onDirty:vi.fn(),onSave:vi.fn()};
 const view=render(<ServiceFromRoute {...props}/>);fireEvent.change(screen.getByLabelText('系統にする路線'),{target:{value:'r'}});
 view.rerender(<ServiceFromRoute {...props} world={{...world}}/>);await act(async()=>resolve(preview));
 expect(screen.queryByLabelText('停車駅 2')).toBeNull();expect(props.onSave).not.toHaveBeenCalled();
});


test('failed creation retains preview and custom name through parent busy toggle for retry',async()=>{
 responses();let reject:(reason:Error)=>void=()=>{};
 const save=vi.fn().mockImplementationOnce(()=>new Promise((_resolve,r)=>{reject=r;})).mockResolvedValue(undefined);
 const props={routes:[{id:'r',name:'本線',start:'a'}],paths:[{route_id:'r',segments}],stations,world,enabled:true,onDirty:vi.fn(),onSave:save};
 const view=render(<ServiceFromRoute {...props}/>);
 fireEvent.change(screen.getByLabelText('系統にする路線'),{target:{value:'r'}});
 await screen.findByLabelText('停車駅 2');
 fireEvent.change(screen.getByLabelText('系統名'),{target:{value:'快速'}});
 fireEvent.click(screen.getByRole('button',{name:'系統を作成'}));
 view.rerender(<ServiceFromRoute {...props} enabled={false}/>);
 await act(async()=>reject(Error('保存失敗')));
 view.rerender(<ServiceFromRoute {...props}/>);
 expect(screen.getByLabelText('系統名')).toHaveProperty('value','快速');
 expect(screen.getByLabelText('停車駅 2')).toHaveProperty('checked',true);
 fireEvent.click(screen.getByRole('button',{name:'系統を作成'}));
 await waitFor(()=>expect(save).toHaveBeenCalledTimes(2));
 expect(save.mock.calls[1][0].name).toBe('快速');
});

test('world epoch remount discards route draft even when another world reuses route id',async()=>{
 responses();
 const props={routes:[{id:'r',name:'本線',start:'a'}],paths:[{route_id:'r',segments}],stations,world,enabled:true,onDirty:vi.fn(),onSave:vi.fn()};
 const view=render(<ServiceFromRoute key="route-service-1-1" {...props}/>);
 fireEvent.change(screen.getByLabelText('系統にする路線'),{target:{value:'r'}});
 await screen.findByLabelText('停車駅 2');
 fireEvent.change(screen.getByLabelText('系統名'),{target:{value:'古い名前'}});
 view.rerender(<ServiceFromRoute key="route-service-2-2" {...props} routes={[{id:'r',name:'別世界線',start:'a'}]} world={{...world}}/>);
 expect(screen.getByLabelText('系統にする路線')).toHaveProperty('value','');
 expect(screen.queryByLabelText('系統名')).toBeNull();
 expect(screen.queryByLabelText('停車駅 2')).toBeNull();
});
