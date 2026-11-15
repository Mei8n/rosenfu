// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
const empty={next_id:'10',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
HTMLDialogElement.prototype.showModal=function(){this.setAttribute('open','');};
HTMLDialogElement.prototype.close=function(){this.removeAttribute('open');};
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();vi.unstubAllGlobals();});
async function start(){
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async command=>{if(command==='engine_info')return {name:'Rosenfu',version:'mode-test'};if(command==='world_snapshot')return empty;if(command==='delete_alignment_section'||command==='delete_station')return empty;if(command==='undo_alignment_deletion')return empty;throw Error(command);});
 render(<App/>);await screen.findByText(/mode-test/);
 fireEvent.click(screen.getByRole('button',{name:'自作サンプル（オフライン）'}));
 const map=screen.getByRole('img',{name:'駅配置用サンプル地図'});map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 fireEvent.click(screen.getByRole('tab',{name:'削除'}));return map;
}

test.each(['delete','tracks','stations'].flatMap(mode=>[130,330].map(stationX=>({mode,stationX}))))('fresh split Undo/Redo preserves station at $stationX after switching to $mode',async ({mode,stationX})=>{
 const map=await start();
 fireEvent.click(screen.getByRole('tab',{name:'線路'}));
 const click=(x:number)=>fireEvent.click(map,{clientX:x,clientY:200,detail:1});
 click(80);click(180);click(280);click(380);click(380);
 fireEvent.click(screen.getByRole('tab',{name:'駅設置'}));click(stationX);
 expect(map.querySelectorAll('.planned-station')).toHaveLength(1);
 fireEvent.click(screen.getByRole('tab',{name:'削除'}));
 fireEvent.click(map.querySelector('[data-delete-plan-section="1"]')!);
 fireEvent.click(screen.getByRole('button',{name:'削除する'}));
 expect(map.querySelectorAll('[data-delete-plan-section]')).toHaveLength(2);
 if(mode!=='delete')fireEvent.click(screen.getByRole('tab',{name:mode==='tracks'?'線路':'駅設置'}));
 const split=Array.from(map.querySelectorAll('.draft-path')).map(p=>p.getAttribute('d'));
 const stationShape=map.querySelector('.planned-station')!.outerHTML;
 for(let cycle=0;cycle<2;cycle++){
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(4);
 expect(map.querySelectorAll('.planned-station')).toHaveLength(1);
 expect(map.querySelectorAll('.draft-path:not(.parked-plan)')).toHaveLength(1);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true,shiftKey:true});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 expect(map.querySelectorAll('.parked-plan')).toHaveLength(1);
 expect(map.querySelectorAll('.planned-station')).toHaveLength(1);
 expect(Array.from(map.querySelectorAll('.draft-path')).map(p=>p.getAttribute('d'))).toEqual(split);
 expect(map.querySelector('.planned-station')!.outerHTML).toBe(stationShape);
 }
});

test('later anchor edit is undone before the split deletion, without losing its station',async()=>{
 vi.stubGlobal('PointerEvent',MouseEvent);
 const map=await start();
 fireEvent.click(screen.getByRole('tab',{name:'線路'}));
 for(const x of [80,180,280,380,380])fireEvent.click(map,{clientX:x,clientY:200,detail:1});
 fireEvent.click(screen.getByRole('tab',{name:'駅設置'}));
 fireEvent.click(map,{clientX:130,clientY:200,detail:1});
 const original=Array.from(map.querySelectorAll('.draft-path')).map(p=>p.getAttribute('d'));
 fireEvent.click(screen.getByRole('tab',{name:'削除'}));
 fireEvent.click(map.querySelector('[data-delete-plan-section="1"]')!);
 fireEvent.click(screen.getByRole('button',{name:'削除する'}));
 fireEvent.click(screen.getByRole('tab',{name:'線路'}));
 const shape=()=>Array.from(map.querySelectorAll('.draft-path')).map(p=>p.getAttribute('d'));
 const split=shape();
 const anchor=map.querySelector('[data-anchor-index="1"]')!;
 fireEvent.pointerDown(anchor,{clientX:180,clientY:200,button:0});
 fireEvent.pointerMove(map,{clientX:180,clientY:220});
 fireEvent.pointerUp(map,{clientX:180,clientY:220});
 expect(shape()).not.toEqual(split);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 expect(shape()).toEqual(split);
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 expect(map.querySelectorAll('.planned-station')).toHaveLength(1);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 expect(shape()).toEqual(original);
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(4);
 expect(map.querySelectorAll('.planned-station')).toHaveLength(1);
});
