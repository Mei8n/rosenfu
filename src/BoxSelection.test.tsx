// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
import {unproject} from './map';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
const alignment={id:'3',owner:'2',name:'線路',points:[[100,300],[500,300],[900,300]].map(([x,y])=>unproject(x,y)),anchor_indices:[0,1,2]};
const empty={next_id:'10',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[],routes:[],services:[],alignments:[],station_placements:[]};
const built={...empty,alignments:[alignment]};
HTMLDialogElement.prototype.showModal=function(){this.setAttribute('open','');};
HTMLDialogElement.prototype.close=function(){this.removeAttribute('open');};
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();vi.unstubAllGlobals();});
async function start(){
 vi.stubGlobal('PointerEvent',MouseEvent);vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async command=>{if(command==='engine_info')return {name:'Rosenfu',version:'box-test'};if(command==='world_snapshot')return built;if(command==='list_save_slots')return {active_slot_id:null,slots:[]};if(command==='delete_alignment_sections')return empty;if(command==='undo_alignment_deletion')return built;throw Error(command);});
 await act(async()=>{render(<App/>);});await screen.findByText(/box-test/);
 const map=screen.getByRole('img',{name:'駅配置用OpenStreetMap'});map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 fireEvent.click(screen.getByRole('tab',{name:'選択'}));return map;
}
function drag(map:HTMLElement,button=0){fireEvent.pointerDown(map,{clientX:100,clientY:140,button});fireEvent.pointerMove(map,{clientX:150,clientY:160,buttons:button===1?4:1});fireEvent.pointerUp(map,{clientX:150,clientY:160,button});}
test('rectangle selects one real section; batch deletion snapshots source and supports undo/redo',async()=>{
 const map=await start();drag(map);expect(map.querySelectorAll('[data-selected-section]')).toHaveLength(1);
 fireEvent.click(screen.getByRole('button',{name:'選択区間を撤去'}));fireEvent.click(screen.getByRole('button',{name:'削除する'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('delete_alignment_sections',expect.objectContaining({selections:[{alignmentId:'3',sectionIndices:[0]}],expectedAlignments:[alignment]})));
 await waitFor(()=>expect(map.querySelector('[data-alignment-id="3"]')).toBeNull());
 await act(async()=>{fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});});await waitFor(()=>expect(invoke).toHaveBeenCalledWith('undo_alignment_deletion'));
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true,shiftKey:true});await waitFor(()=>expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==='delete_alignment_sections')).toHaveLength(2));
});
test('middle drag pans instead of selecting or building and pointer cancel does not select',async()=>{
 const map=await start();const before=map.querySelector('.alignment-hit')?.getAttribute('d');drag(map,1);
 expect(map.querySelector('.alignment-hit')?.getAttribute('d')).not.toBe(before);expect(map.querySelector('[data-selected-section]')).toBeNull();
 fireEvent.pointerDown(map,{clientX:100,clientY:140,button:0});fireEvent.pointerMove(map,{clientX:150,clientY:160});fireEvent.pointerCancel(map);expect(map.querySelector('[data-selected-section]')).toBeNull();
 expect(vi.mocked(invoke).mock.calls.map(([c])=>c)).toEqual(['engine_info','world_snapshot','list_save_slots']);
});
