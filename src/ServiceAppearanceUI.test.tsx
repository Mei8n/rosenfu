// @vitest-environment jsdom
import React from 'react';
import {act,cleanup,fireEvent,render,screen,waitFor,within} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
const segment={kind:'alignment',alignment_id:'9',from:'3',to:'5'};
const path={service_id:'6',next_leg_key:'2',legs:[{key:'1',segment}],stops:[{kind:'start'},{kind:'on_leg',leg_key:'1',station_id:'5'}]};
const base={next_id:'10',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:['3','4','5'].map((id,i)=>({id,owner:'2',name:['起点','途中','終点'][i],latitude:35,longitude:139+i*.01})),routes:[],route_paths:[],services:[{id:'6',owner:'2',name:'普通',start:'3'}],service_paths:[path],service_route_bindings:[{service_id:'6',binding:{kind:'independent'}}],service_appearances:[{service_id:'6',color:'#123456',shape:'circle',symbol:'普'}],station_placements:['3','4','5'].map((station_id,i)=>({station_id,alignment_id:'9',position:i/2,length_m:100,width_m:12}))};
async function setup(){let world=structuredClone(base);vi.mocked(isTauri).mockReturnValue(true);vi.mocked(invoke).mockImplementation(async(c,args)=>{
 if(c==='engine_info')return {name:'Rosenfu',version:'appearance-ui-test'};if(c==='world_snapshot')return world;
 if(c==='list_save_slots')return {active_slot_id:'slot',slots:[]};if(c==='autosave_status')return {available:false};
 if(c==='duplicate_service'){world={...world,services:[...world.services,{...world.services[0],id:'10',name:'普通 のコピー'}],service_paths:[...world.service_paths,{...structuredClone(path),service_id:'10'}],service_route_bindings:[...world.service_route_bindings,{service_id:'10',binding:{kind:'independent'}}],service_appearances:[...world.service_appearances,{...world.service_appearances[0],service_id:'10'}]};return world;}
 if(c==='update_service_path'){const a=args as {serviceId:string;name:string;stops:typeof path.stops};world={...world,services:world.services.map(s=>s.id===a.serviceId?{...s,name:a.name}:s),service_paths:world.service_paths.map(p=>p.service_id===a.serviceId?{...p,stops:a.stops}:p)};return world;}
 if(c==='set_service_appearance'){const a=args as {serviceId:string;color:string;shape:string;symbol:string};world={...world,service_appearances:world.service_appearances.map(p=>p.service_id===a.serviceId?{...p,color:a.color.toUpperCase(),shape:a.shape,symbol:a.symbol}:p)};return world;}
 throw Error(String(c));});
 render(<App/>);await screen.findByText(/appearance-ui-test/);await waitFor(()=>expect(invoke).toHaveBeenCalledWith('autosave_status',{slotId:'slot'}));await act(async()=>{});fireEvent.click(screen.getByRole('tab',{name:'系統'}));fireEvent.click(screen.getByRole('button',{name:'系統編集: 普通'}));return ()=>world;
}
test('duplicate selects independent new service; editing its name preserves original path, name and colour',async()=>{
 const snapshot=await setup();fireEvent.click(screen.getByRole('button',{name:'系統を複製'}));await screen.findByRole('button',{name:'系統編集: 普通 のコピー'});
 expect(invoke).toHaveBeenCalledWith('duplicate_service',{serviceId:'6',expectedPath:path,expectedName:'普通'});
 expect((screen.getByLabelText('保存済み系統名') as HTMLInputElement).value).toBe('普通 のコピー');
 fireEvent.change(screen.getByLabelText('保存済み系統名'),{target:{value:'急行'}});fireEvent.click(screen.getByRole('button',{name:'保存済み系統の変更を適用'}));await screen.findByRole('button',{name:'系統編集: 急行'});
 expect(snapshot().services[0]).toEqual(base.services[0]);expect(snapshot().service_paths[0]).toEqual(path);expect(snapshot().service_appearances[0]).toEqual(base.service_appearances[0]);
 expect(snapshot().services[1].id).toBe('10');expect(snapshot().services[1].name).toBe('急行');
});
test('colour draft blocks switching, save and other editor until applied; matching result clears draft',async()=>{
 await setup();fireEvent.change(screen.getByLabelText('系統色'),{target:{value:'#aabbcc'}});
 fireEvent.click(screen.getByRole('button',{name:'新規系統'}));expect(screen.getByText(/先に系統色の変更/)).toBeTruthy();
 expect((screen.getByLabelText('保存済み系統名') as HTMLInputElement).closest('fieldset')?.disabled).toBe(true);
 expect((screen.getByRole('button',{name:'保存'}) as HTMLButtonElement).disabled).toBe(true);
 fireEvent.click(screen.getByRole('button',{name:'色を適用'}));await screen.findByText(/系統色を適用しました/);
 expect(invoke).toHaveBeenCalledWith('set_service_appearance',{serviceId:'6',color:'#aabbcc',shape:'circle',symbol:'普',expectedAppearance:base.service_appearances[0]});
 await waitFor(()=>expect((screen.getByRole('button',{name:'保存'}) as HTMLButtonElement).disabled).toBe(false));
 expect((screen.getByLabelText('系統色') as HTMLInputElement).value).toBe('#aabbcc');
 expect(screen.queryByText(/保存済みの色が変更/)).toBeNull();
 expect((screen.getByRole('button',{name:'系統を複製'}) as HTMLButtonElement).disabled).toBe(false);
});
