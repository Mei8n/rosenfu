// @vitest-environment jsdom
import {Profiler} from 'react';
let renders=0;
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
import {emptyConstructionPlans} from './constructionPlans';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
const a={latitude:35.67504,longitude:139.72},b={latitude:35.67504,longitude:139.75},c={latitude:35.67504,longitude:139.78};
const empty={next_id:'10',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
const alignment={id:'3',owner:'2',name:'線路',points:[a,b,c],anchor_indices:[0,1,2]};
const built={...empty,alignments:[alignment]};
HTMLDialogElement.prototype.showModal=function(){this.setAttribute('open','');};
HTMLDialogElement.prototype.close=function(){this.removeAttribute('open');};
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
async function start(snapshot:unknown=built){
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async command=>{if(command==='engine_info')return {name:'Rosenfu',version:'mode-test'};if(command==='world_snapshot')return snapshot;if(command==='delete_alignment_section'||command==='delete_station')return empty;if(command==='undo_alignment_deletion')return snapshot;throw Error(command);});
 render(<Profiler id="map" onRender={()=>renders++}><App/></Profiler>);await screen.findByText(/mode-test/);
 const map=screen.getByRole('img',{name:'駅配置用OpenStreetMap'});map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 fireEvent.click(screen.getByRole('tab',{name:'削除'}));return map;
}
test('moving within a deletion target avoids repeated map renders',async()=>{
 const lines=Array.from({length:40},(_,i)=>({...alignment,id:String(i+100),points:Array.from({length:60},(_,j)=>({latitude:35.67504+i*.001,longitude:139.72+j*.0001})),anchor_indices:[0,20,40,59]}));
 const map=await start({...empty,alignments:lines});
 const target=map.querySelector('[data-delete-alignment]')!;
 fireEvent.pointerEnter(target,{clientX:200,clientY:150});
 const before=renders;
 for(let i=0;i<30;i++)fireEvent.pointerMove(target,{clientX:200+i,clientY:150,pointerType:'mouse'});
 expect(renders-before).toBeLessThanOrEqual(1);
 expect(target.classList.contains('deletion-hover')).toBe(true);
 const other=map.querySelectorAll('[data-delete-alignment]')[1];
 fireEvent.pointerEnter(other,{clientX:240,clientY:150});
 expect(other.classList.contains('deletion-hover')).toBe(true);
 expect(target.classList.contains('deletion-hover')).toBe(false);
 fireEvent.pointerLeave(other);
 expect(other.classList.contains('deletion-hover')).toBe(false);
});
