// @vitest-environment jsdom
import React from 'react';
import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {ServicePathEditor} from './ServicePathEditor';
import {routeSection} from './serviceSection';
import {resolveServicePath} from './servicePaths';
afterEach(cleanup);
const world={tracks:[],station_placements:['a','b','c','d'].map((station_id,i)=>({station_id,alignment_id:'line',position:i/3}))};
const segment={kind:'alignment' as const,alignment_id:'line',from:'a',to:'d'};
const legs=[{key:'7',segment}],occurrences=resolveServicePath('a',legs,world);
const path={service_id:'service',next_leg_key:'8',legs,stops:occurrences.map(o=>o.reference)};
const props={service:{id:'service',name:'通勤',start:'a'},path,world,stations:['a','b','c','d'].map(id=>({id,name:id})),enabled:true,onDirty:vi.fn(),onSave:vi.fn()};
test('saved service editing preserves failed edits, supports cancellation and stable stop references',async()=>{
 const save=vi.fn().mockRejectedValueOnce(Error('拒否')).mockResolvedValueOnce(undefined);render(<ServicePathEditor {...props} onSave={save}/>);
 expect((screen.getByLabelText('保存済み停車 1') as HTMLInputElement).disabled).toBe(true);
 fireEvent.change(screen.getByLabelText('保存済み系統名'),{target:{value:'快速'}});fireEvent.click(screen.getByLabelText('保存済み停車 2'));fireEvent.click(screen.getByRole('button',{name:'保存済み系統の変更を適用'}));await screen.findByRole('alert');
 expect((screen.getByLabelText('保存済み系統名') as HTMLInputElement).value).toBe('快速');expect(save.mock.calls[0]).toEqual(['快速',[occurrences[0].reference,occurrences[2].reference,occurrences[3].reference]]);
 fireEvent.click(screen.getByRole('button',{name:'保存済み系統の変更を取り消す'}));expect((screen.getByLabelText('保存済み系統名') as HTMLInputElement).value).toBe('通勤');expect((screen.getByLabelText('保存済み停車 2') as HTMLInputElement).checked).toBe(true);
 fireEvent.click(screen.getByLabelText('保存済み停車 3'));fireEvent.click(screen.getByRole('button',{name:'保存済み系統の変更を適用'}));await waitFor(()=>expect(save).toHaveBeenCalledTimes(2));expect(save.mock.calls[1][1][1]).toEqual({kind:'on_leg',leg_key:'7',station_id:'b'});
});
test('partial route slices preserve forward/reverse ordered stations and reject equal/out of range positions',()=>{
 expect(routeSection('a',[segment],1,3,world)).toEqual({start:'b',segments:[{...segment,from:'b',to:'d'}]});expect(routeSection('a',[segment],2,0,world)).toEqual({start:'c',segments:[{...segment,from:'c',to:'a'}]});expect(()=>routeSection('a',[segment],1,1,world)).toThrow();expect(()=>routeSection('a',[segment],0,4,world)).toThrow();
});
test('connected partial route keeps only crossed links and handles reverse direction',()=>{
 const link={a:{alignment_id:'L',endpoint:'end' as const},b:{alignment_id:'R',endpoint:'start' as const}};
 const w={tracks:[],alignment_connections:[link],station_placements:[{station_id:'a',alignment_id:'L',position:.1},{station_id:'b',alignment_id:'L',position:.7},{station_id:'c',alignment_id:'R',position:.3},{station_id:'d',alignment_id:'R',position:.9}]};
 const s={kind:'connected_alignments' as const,from:'a',to:'d',connections:[link]};
 expect(routeSection('a',[s],0,1,w).segments).toEqual([{kind:'alignment',alignment_id:'L',from:'a',to:'b'}]);expect(routeSection('a',[s],2,1,w).segments).toEqual([{...s,from:'c',to:'b',connections:[{a:link.b,b:link.a}]}]);
});

test('unrelated world snapshots preserve unsaved service fields and changed saved data is a visible conflict',()=>{
 const view=render(<ServicePathEditor {...props}/>);fireEvent.change(screen.getByLabelText('保存済み系統名'),{target:{value:'編集中'}});fireEvent.click(screen.getByLabelText('保存済み停車 2'));
 view.rerender(<ServicePathEditor {...props} world={{...world}} path={JSON.parse(JSON.stringify(path))}/>);expect((screen.getByLabelText('保存済み系統名') as HTMLInputElement).value).toBe('編集中');expect((screen.getByLabelText('保存済み停車 2') as HTMLInputElement).checked).toBe(false);
 view.rerender(<ServicePathEditor {...props} service={{...props.service,name:'他の変更'}}/>);expect(screen.getByRole('alert').textContent).toContain('入力は保持');expect((screen.getByRole('button',{name:'保存済み系統の変更を適用'}) as HTMLButtonElement).disabled).toBe(true);
 fireEvent.click(screen.getByRole('button',{name:'保存済み系統の変更を取り消す'}));expect((screen.getByLabelText('保存済み系統名') as HTMLInputElement).value).toBe('他の変更');
});
