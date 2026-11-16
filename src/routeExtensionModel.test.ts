import {expect,test} from 'vitest';
import {extensionAvailable,extensionStops} from './routeExtensionModel';
import type {ServicePath,PathPreview} from './servicePaths';
const a={kind:'alignment' as const,alignment_id:'a',from:'1',to:'3'};
const b={...a,from:'3',to:'4'};
const path:ServicePath={service_id:'s',next_leg_key:'9',legs:[{key:'7',segment:a}],stops:[{kind:'start'},{kind:'on_leg',leg_key:'7',station_id:'3'}]};
const world={station_placements:['1','2','3','4'].map((station_id,i)=>({station_id,alignment_id:'a',position:i/3}))};
const preview:PathPreview={legs:[{key:'9',segment:a},{key:'10',segment:b}],occurrences:[],requires_stop_selection:true};
test('extension keeps old physical prefix and rejects reroutes or shortening',()=>{
 expect(extensionAvailable('1',path,{start:'1',segments:[a,b]})).toBe(true);
 expect(extensionAvailable('1',path,{start:'1',segments:[a]})).toBe(false);
 expect(extensionAvailable('1',path,{start:'2',segments:[a,b]})).toBe(false);
 expect(extensionAvailable('1',path,{start:'1',segments:[{...a,alignment_id:'other'},b]})).toBe(false);
});
test('express pass settings survive new leg keys and additional stations default to stop',()=>{
 expect(extensionStops('1',path,preview,world)).toEqual([{kind:'start'},{kind:'on_leg',leg_key:'9',station_id:'3'},{kind:'on_leg',leg_key:'10',station_id:'4'}]);
});
test('changed station order is refused',()=>{
 expect(()=>extensionStops('1',path,{...preview,legs:[{key:'9',segment:{...a,to:'2'}}]},world)).toThrow();
});
