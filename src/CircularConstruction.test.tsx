// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
import {project} from './map';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
test('circular preview matches saved geometry and undo/redo restores guide',async()=>{
 const world={next_id:'3',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'R'}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async command=>command==='engine_info'?{name:'Rosenfu',version:'circle-test'}:world);
 render(<App/>);fireEvent.click(screen.getByRole('button',{name:'自作サンプル（オフライン）'}));await screen.findByText(/circle-test/);
 const map=screen.getByRole('img',{name:'駅配置用サンプル地図'});
 map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 fireEvent.click(map,{clientX:180,clientY:130});fireEvent.click(map,{clientX:250,clientY:130});fireEvent.click(map,{clientX:300,clientY:180});
 expect(map.querySelector('.curve-circle-guide')).not.toBeNull();
 const preview=map.querySelector('.draft-path')!.getAttribute('d');
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelector('.curve-circle-guide')).toBeNull();
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(map.querySelector('.draft-path')!.getAttribute('d')).toBe(preview);
 fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));
 const call=vi.mocked(invoke).mock.calls.find(([command])=>command==='build_alignment')!;
 expect(call).toBeTruthy();
 const points=(call[1] as {points:{latitude:number;longitude:number}[]}).points;
 expect(points.map((point,index)=>{const p=project(point.latitude,point.longitude);return(index?'L':'M')+p.x+','+p.y;}).join(' ')).toBe(preview);
});
