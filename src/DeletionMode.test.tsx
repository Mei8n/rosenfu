// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
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
 render(<App/>);await screen.findByText(/mode-test/);
 const map=screen.getByRole('img',{name:'駅配置用OpenStreetMap'});map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 fireEvent.click(screen.getByRole('tab',{name:'削除'}));return map;
}
test('dedicated deletion mode highlights only selected real anchor interval, cancels then deletes and undoes',async()=>{
 const map=await start();const targets=map.querySelectorAll('[data-delete-alignment="3"]');expect(targets).toHaveLength(2);expect(targets[0].getAttribute("fill")).toBe("none");expect(targets[0].getAttribute("pointer-events")).toBe("stroke");
 fireEvent.pointerEnter(targets[1],{clientX:200,clientY:150});expect(targets[1].classList.contains('deletion-hover')).toBe(true);expect(targets[0].classList.contains('deletion-hover')).toBe(false);
 fireEvent.click(targets[1]);expect(screen.getByRole('dialog')).toBeTruthy();fireEvent.click(screen.getByRole('button',{name:'キャンセル'}));expect(invoke).not.toHaveBeenCalledWith('delete_alignment_section',expect.anything());
 fireEvent.click(targets[1]);fireEvent.click(screen.getByRole('button',{name:'削除する'}));await waitFor(()=>expect(map.querySelector('[data-delete-alignment]')).toBeNull());
 expect(invoke).toHaveBeenCalledWith('delete_alignment_section',expect.objectContaining({alignmentId:'3',sectionIndex:1,constructionPlans:expect.any(Object)}));
 fireEvent.click(screen.getByRole('button',{name:'削除を元に戻す'}));await waitFor(()=>expect(map.querySelectorAll('[data-delete-alignment]')).toHaveLength(2));
});
test('individual plan anchor deletion leaves a finished singleton and supports undo',async()=>{
 const map=await start({...empty,construction_plans:{...emptyConstructionPlans(),draftPoints:[a,b],draftFinished:true}});
 fireEvent.click(map.querySelector('[data-delete-anchor="1"]')!);fireEvent.click(screen.getByRole('button',{name:'削除する'}));expect(map.querySelectorAll('[data-delete-anchor]')).toHaveLength(1);expect(map.querySelector('.candidate-draft')).toBeNull();
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelectorAll('[data-delete-anchor]')).toHaveLength(2);
 expect(vi.mocked(invoke).mock.calls.some(([cmd])=>cmd==='build_plan'||cmd==='delete_alignment_section')).toBe(false);
});
test('built stations are removable through the same map mode',async()=>{
 const map=await start({...empty,stations:[{id:'4',owner:'2',name:'駅',...b}]});fireEvent.click(map.querySelector('[data-delete-station="4"]')!);fireEvent.click(screen.getByRole('button',{name:'削除する'}));
 await waitFor(()=>expect(map.querySelector('[data-delete-station]')).toBeNull());expect(invoke).toHaveBeenCalledWith('delete_station',expect.objectContaining({stationId:'4',constructionPlans:expect.any(Object)}));
});
test('referenced track cannot be deleted and error does not mutate anything',async()=>{
 const map=await start({...built,stations:[{id:'4',owner:'2',name:'駅',...b}],station_placements:[{station_id:'4',alignment_id:'3',position:.5,length_m:200,width_m:12}]});
 fireEvent.click(map.querySelector('[data-delete-section="0"]')!);expect(screen.getByRole('alert').textContent).toContain('駅が使用中');expect((screen.getByRole('button',{name:'削除する'}) as HTMLButtonElement).disabled).toBe(true);
});
