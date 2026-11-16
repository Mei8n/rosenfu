// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {ServiceRouteExtension} from './ServiceRouteExtension';
import {resolveServicePath,type ServicePath,type PathPreview} from './servicePaths';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
const a={kind:'alignment' as const,alignment_id:'a',from:'1',to:'3'},b={...a,from:'3',to:'4'};
const path:ServicePath={service_id:'s',next_leg_key:'9',legs:[{key:'7',segment:a}],stops:[{kind:'start'},{kind:'on_leg',leg_key:'7',station_id:'3'}]};
const world={station_placements:['1','2','3','4'].map((station_id,i)=>({station_id,alignment_id:'a',position:i/3}))};
const legs=[{key:'9',segment:a},{key:'10',segment:b}];
const preview:PathPreview={legs,occurrences:resolveServicePath('1',legs,world),requires_stop_selection:true};
function setup(){const props={service:{id:'s',name:'急行',start:'1'},path,route:{id:'r',name:'本線',start:'1',segments:[a,b]},world,stations:['1','2','3','4'].map(id=>({id,name:`駅${id}`})),enabled:true,onDirty:vi.fn(),onSave:vi.fn().mockResolvedValue(undefined)};vi.mocked(invoke).mockResolvedValue(preview);return {...render(<ServiceRouteExtension {...props}/>),props};}
test('preview preserves express stops; applies only explicit choice, retains name and route binding',async()=>{
 const h=setup();fireEvent.click(screen.getByText('追加する駅を確認'));await screen.findByLabelText('延伸後の停車駅 4');
 expect((screen.getByLabelText('延伸後の停車駅 2') as HTMLInputElement).checked).toBe(false);
 expect((screen.getByLabelText('延伸後の停車駅 4') as HTMLInputElement).checked).toBe(true);expect(h.props.onSave).not.toHaveBeenCalled();
 fireEvent.click(screen.getByText('この系統を延伸'));await waitFor(()=>expect(h.props.onSave).toHaveBeenCalledOnce());
 expect(h.props.onSave.mock.calls[0][0]).toMatchObject({name:'急行',sourceRouteId:'r',start:'1',segments:[a,b],stops:[{kind:'start'},{kind:'on_leg',leg_key:'9',station_id:'3'},{kind:'on_leg',leg_key:'10',station_id:'4'}]});
});
test('cancel does not save and failure keeps the editable preview',async()=>{
 const h=setup();h.props.onSave.mockRejectedValue(Error('競合'));fireEvent.click(screen.getByText('追加する駅を確認'));await screen.findByLabelText('延伸後の停車駅 4');fireEvent.click(screen.getByText('この系統を延伸'));await screen.findByRole('alert');expect(screen.getByLabelText('延伸後の停車駅 4')).toBeTruthy();fireEvent.click(screen.getByText('キャンセル'));expect(h.props.onSave).toHaveBeenCalledOnce();expect(h.props.onDirty).toHaveBeenLastCalledWith(false);
});
test('world replacement rejects late preview and does not save',async()=>{
 const h=setup();let done!:(p:PathPreview)=>void;vi.mocked(invoke).mockImplementationOnce(()=>new Promise(r=>{done=r;}));fireEvent.click(screen.getByText('追加する駅を確認'));h.rerender(<ServiceRouteExtension {...h.props} world={{...world}}/>);await act(async()=>done(preview));expect(screen.queryByText('この系統を延伸')).toBeNull();expect(h.props.onSave).not.toHaveBeenCalled();
});
test('competing draft disables extension',()=>{const h=setup();h.rerender(<ServiceRouteExtension {...h.props} enabled={false}/>);expect((screen.getByText('追加する駅を確認') as HTMLButtonElement).disabled).toBe(true);});

test('pending preview locks other drafts immediately and releases lock on failure',async()=>{
 const h=setup();let fail!:(reason:Error)=>void;vi.mocked(invoke).mockImplementationOnce(()=>new Promise((_,reject)=>{fail=reject;}));
 fireEvent.click(screen.getByText('追加する駅を確認'));expect(h.props.onDirty).toHaveBeenLastCalledWith(true);
 await act(async()=>fail(Error('失敗')));expect(h.props.onDirty).toHaveBeenLastCalledWith(false);expect(screen.queryByText('この系統を延伸')).toBeNull();
});

test('world changes retain unapplied stop choices as stale until explicit cancellation',async()=>{
 const h=setup();fireEvent.click(screen.getByText('追加する駅を確認'));await screen.findByLabelText('延伸後の停車駅 4');
 fireEvent.click(screen.getByLabelText('延伸後の停車駅 2'));h.rerender(<ServiceRouteExtension {...h.props} world={{...world}}/>);
 expect((screen.getByLabelText('延伸後の停車駅 2') as HTMLInputElement).checked).toBe(true);expect(screen.getByRole('alert').textContent).toContain('入力は保持');
 expect((screen.getByText('この系統を延伸') as HTMLButtonElement).disabled).toBe(true);expect(h.props.onDirty).toHaveBeenLastCalledWith(true);
 fireEvent.click(screen.getByText('キャンセル'));expect(h.props.onDirty).toHaveBeenLastCalledWith(false);expect(h.props.onSave).not.toHaveBeenCalled();
});
