// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
import {emptyConstructionPlans} from './constructionPlans';
import {unproject} from './map';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
const alignment={id:'3',owner:'2',name:'線路',points:[[100,300],[500,300],[900,300]].map(([x,y])=>unproject(x,y)),anchor_indices:[0,1,2]};
const empty={next_id:'10',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[],routes:[],services:[],alignments:[],station_placements:[]};
const built={...empty,alignments:[alignment]};
HTMLDialogElement.prototype.showModal=function(){this.setAttribute('open','');};
HTMLDialogElement.prototype.close=function(){this.removeAttribute('open');};
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();vi.unstubAllGlobals();});
async function start(initial:object=built){
 vi.stubGlobal('PointerEvent',MouseEvent);vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async (command,args)=>{if(command==='engine_info')return {name:'Rosenfu',version:'box-test'};if(command==='world_snapshot')return initial;if(command==='list_save_slots')return {active_slot_id:null,slots:[]};if(command==='delete_alignment_sections')return empty;if(command==='undo_world_edit'||command==='redo_world_edit')return {...(command==='undo_world_edit'?built:empty),construction_plans:(args as any).expectedWorld.construction_plans,view_state:(args as any).expectedWorld.view_state};throw Error(command);});
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
 await act(async()=>{fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});});await waitFor(()=>expect(invoke).toHaveBeenCalledWith('undo_world_edit',expect.objectContaining({expectedWorld:expect.any(Object)})));expect(map.querySelector('[data-alignment-id="3"]')).not.toBeNull();
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true,shiftKey:true});await waitFor(()=>expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==='redo_world_edit')).toHaveLength(1));await waitFor(()=>expect(map.querySelector('[data-alignment-id="3"]')).toBeNull());expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==='delete_alignment_sections')).toHaveLength(1);
});
test('middle drag pans instead of selecting or building and pointer cancel does not select',async()=>{
 const map=await start();const before=map.querySelector('.alignment-hit')?.getAttribute('d');drag(map,1);
 expect(map.querySelector('.alignment-hit')?.getAttribute('d')).not.toBe(before);expect(map.querySelector('[data-selected-section]')).toBeNull();
 fireEvent.pointerDown(map,{clientX:100,clientY:140,button:0});fireEvent.pointerMove(map,{clientX:150,clientY:160});fireEvent.pointerCancel(map);expect(map.querySelector('[data-selected-section]')).toBeNull();
 await waitFor(()=>expect(vi.mocked(invoke).mock.calls.map(([c])=>c)).toEqual(['engine_info','world_snapshot','water_mask_requests','list_save_slots','water_mask_requests']));
});
test('infrastructure change keeps unspecified field, snapshots selected alignment, and retains rejected edits',async()=>{
 const map=await start();drag(map);
 expect(screen.getByLabelText('選択区間の線路数').textContent).toBe('単線');
 const count=screen.getByLabelText('選択区間の線路数変更');fireEvent.change(count,{target:{value:'4'}});
 vi.mocked(invoke).mockImplementation(async (command,args)=>{if(command==='update_alignment_section_infrastructure')throw Error('更新競合');throw Error(command);});
 fireEvent.click(screen.getByRole('button',{name:'選択区間へ適用'}));
 await screen.findByText(/更新競合/);expect((count as HTMLSelectElement).value).toBe('4');expect(map.querySelectorAll('[data-selected-section]')).toHaveLength(1);
 expect(invoke).toHaveBeenCalledWith('update_alignment_section_infrastructure',expect.objectContaining({trackCount:4,level:null,selections:[{alignmentId:'3',sectionIndices:[0]}],expectedAlignments:[alignment]}));
 const changed={...built,alignments:[{...alignment,infrastructure:[{track_count:4,level:'underground_3'},{track_count:1,level:'ground'}]}]};
 vi.mocked(invoke).mockImplementation(async (command,args)=>command==='update_alignment_section_infrastructure'?changed:built);
 fireEvent.change(screen.getByLabelText('選択区間の高さ変更'),{target:{value:'underground_3'}});
 await act(async()=>{fireEvent.click(screen.getByRole('button',{name:'選択区間へ適用'}));});
 expect(map.querySelector('[data-selected-section]')).toBeNull();
 await act(async()=>{fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});});expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='undo_world_edit'||command==='undo_alignment_deletion')).toBe(false);
});
test.each(['線路','駅設置','削除'])('middle pan does not place or delete in %s mode',async mode=>{
 const map=await start();fireEvent.click(screen.getByRole('tab',{name:mode}));const before=map.querySelector('.alignment-hit')?.getAttribute('d');drag(map,1);
 expect(map.querySelector('.alignment-hit')?.getAttribute('d')).not.toBe(before);expect(screen.queryByRole('dialog')).toBeNull();
 await waitFor(()=>expect(vi.mocked(invoke).mock.calls.map(([c])=>c)).toEqual(['engine_info','world_snapshot','water_mask_requests','list_save_slots','water_mask_requests']));
 if(mode==='削除')expect(map.classList.contains('map-delete-mode')).toBe(true);
});
test.each(['keyboard','wheel'])('zoom cancels in-flight rectangle before camera changes: %s',async kind=>{
 const map=await start();fireEvent.pointerDown(map,{clientX:100,clientY:140,button:0});fireEvent.pointerMove(map,{clientX:150,clientY:160});expect(map.querySelector('.selection-box')).not.toBeNull();
 if(kind==='keyboard')fireEvent.keyDown(document.body,{key:'+'});else fireEvent.wheel(map,{clientX:250,clientY:150,deltaY:-100});
 expect(map.querySelector('.selection-box')).toBeNull();fireEvent.pointerUp(map,{clientX:150,clientY:160});expect(map.querySelector('[data-selected-section]')).toBeNull();expect(screen.getByRole('button',{name:'選択区間を撤去'}).hasAttribute('disabled')).toBe(true);
});
test('Delete opens confirmation only for an idle selection and does not remove before confirmation',async()=>{
 const map=await start();fireEvent.keyDown(document.body,{key:'Delete'});expect(screen.queryByRole('dialog')).toBeNull();
 drag(map);
 for(const options of [{ctrlKey:true},{altKey:true},{metaKey:true},{shiftKey:true},{repeat:true},{isComposing:true},{keyCode:229}]){fireEvent.keyDown(document.body,{key:'Delete',...options});expect(screen.queryByRole('dialog')).toBeNull();}
 for(const tag of ['input','textarea','select']){const input=document.createElement(tag);document.body.append(input);fireEvent.keyDown(input,{key:'Delete'});input.remove();expect(screen.queryByRole('dialog')).toBeNull();}
 fireEvent.keyDown(document.body,{key:'Delete'});expect(screen.getByRole('dialog')).toBeTruthy();expect(invoke).not.toHaveBeenCalledWith('delete_alignment_sections',expect.anything());
 fireEvent.click(screen.getByRole('button',{name:'キャンセル'}));expect(map.querySelectorAll('[data-selected-section]')).toHaveLength(1);
 fireEvent.click(screen.getByRole('tab',{name:'線路'}));fireEvent.keyDown(document.body,{key:'Delete'});expect(screen.queryByRole('dialog')).toBeNull();
});
function rectangle(map:HTMLElement,x:number,y:number,shiftKey=false){fireEvent.pointerDown(map,{clientX:x,clientY:y,button:0,shiftKey});fireEvent.pointerMove(map,{clientX:x+30,clientY:y+20,buttons:1});fireEvent.pointerUp(map,{clientX:x+30,clientY:y+20,button:0});}
test('Shift adds distinct sections, preserves empty/canceled additions, and Delete targets the union',async()=>{
 const map=await start();drag(map);rectangle(map,300,140,true);expect(map.querySelectorAll('[data-selected-section]')).toHaveLength(2);
 rectangle(map,100,140,true);rectangle(map,100,40,true);expect(map.querySelectorAll('[data-selected-section]')).toHaveLength(2);
 fireEvent.pointerDown(map,{clientX:100,clientY:40,button:0,shiftKey:true});fireEvent.pointerCancel(map);expect(map.querySelectorAll('[data-selected-section]')).toHaveLength(2);
 fireEvent.keyDown(document.body,{key:'Delete'});fireEvent.click(screen.getByRole('button',{name:'削除する'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('delete_alignment_sections',expect.objectContaining({selections:[{alignmentId:'3',sectionIndices:[0,1]}]})));
});
test('normal rectangle replaces after Shift addition, and mode switch clears selection and gesture',async()=>{
 const map=await start();drag(map);rectangle(map,300,140,true);rectangle(map,300,140);expect(map.querySelectorAll('[data-selected-section]')).toHaveLength(1);expect(map.querySelector('[data-selected-section]')?.getAttribute('data-selected-section')).toBe('1');
 rectangle(map,100,40);expect(map.querySelectorAll('[data-selected-section]')).toHaveLength(0);
 drag(map);fireEvent.pointerDown(map,{clientX:300,clientY:140,button:0,shiftKey:true});fireEvent.keyDown(document.body,{key:'Delete'});expect(screen.queryByRole('dialog')).toBeNull();
 fireEvent.click(screen.getByRole('tab',{name:'線路'}));fireEvent.pointerUp(map,{clientX:330,clientY:160,button:0});fireEvent.click(screen.getByRole('tab',{name:'選択'}));expect(map.querySelectorAll('[data-selected-section]')).toHaveLength(0);
});

test('Delete confirmation clears a previous action error just like the delete button',async()=>{
 const map=await start();drag(map);
 fireEvent.change(screen.getByLabelText('選択区間の線路数変更'),{target:{value:'4'}});
 vi.mocked(invoke).mockImplementation(async command=>{throw Error(command==='update_alignment_section_infrastructure'?'更新競合':command);});
 // Flush the rejected operation and React's key-handler effect before retrying.
 await act(async()=>{fireEvent.click(screen.getByRole('button',{name:'選択区間へ適用'}));});
 await screen.findByText(/更新競合/);
 await waitFor(()=>expect((screen.getByRole('button',{name:'選択区間を撤去'}) as HTMLButtonElement).disabled).toBe(false));
 fireEvent.keyDown(document.body,{key:'Delete'});
 expect(screen.getByRole('dialog')).toBeTruthy();expect(screen.queryByText(/更新競合/)).toBeNull();
 expect(invoke).not.toHaveBeenCalledWith('delete_alignment_sections',expect.anything());
});

test('built track category is applied explicitly and is a final construction operation',async()=>{
 const map=await start();drag(map);
 const apply=screen.getByRole('button',{name:'線路種別を適用'});expect(apply.hasAttribute('disabled')).toBe(true);
 fireEvent.change(screen.getByLabelText('選択区間の線路種別変更'),{target:{value:'high_speed'}});
 const changed={...built,alignments:[{...alignment,track_type:'high_speed'}]};
 vi.mocked(invoke).mockImplementation(async (command,args)=>command==='update_alignment_section_track_type'?changed:built);
 await act(async()=>{fireEvent.click(apply);});
 expect(invoke).toHaveBeenCalledWith('update_alignment_section_track_type',expect.objectContaining({trackType:'high_speed',selections:[{alignmentId:'3',sectionIndices:[0]}],expectedAlignments:[alignment]}));
 expect(map.querySelector('[data-selected-section]')).toBeNull();
 await act(async()=>{fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});});
 expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='undo_world_edit'||command==='undo_alignment_deletion')).toBe(false);
 await act(async()=>{fireEvent.keyDown(document.body,{key:'z',ctrlKey:true,shiftKey:true});});
 expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==='update_alignment_section_track_type')).toHaveLength(1);
});

test('final category split keeps remapped pending extension targets through save and attempted undo redo',async()=>{
 const endpoint={alignmentId:'3',endpoint:'end' as const};
 const parked={id:2,planId:1,name:'保留線',points:[unproject(950,350),alignment.points[2]],line:[unproject(950,350),alignment.points[2]],redo:[],extension:null,destination:endpoint,redoDestination:endpoint,designSpeedKmh:120,trackType:'normal' as const,roadFollowing:false};
 const original={...emptyConstructionPlans(),draftPoints:[alignment.points[2],unproject(1000,300)],draftExtension:endpoint,draftFinished:true,parkedPaths:[parked]};
 const before={...built,construction_plans:original};
 const map=await start(before);drag(map);
 const remappedEndpoint={alignmentId:'30',endpoint:'end' as const};
 const remapped={...original,draftExtension:remappedEndpoint,parkedPaths:[{...parked,destination:remappedEndpoint,redoDestination:remappedEndpoint}]};
 const after={...built,alignments:[alignment,{...alignment,id:'30',track_type:'high_speed'}],construction_plans:remapped};
 vi.mocked(invoke).mockImplementation(async (command,args)=>{
  if(command==='update_alignment_section_track_type')return after;
  if(command==='undo_world_edit'||command==='redo_world_edit')throw Error('final construction must not invoke history');
  if(command==='save_world')return {saved:true};
  if(command==='list_save_slots')return {active_slot_id:'test-slot',slots:[]};
  throw Error(command);
 });
 fireEvent.change(screen.getByLabelText('選択区間の線路種別変更'),{target:{value:'high_speed'}});
 await act(async()=>{fireEvent.click(screen.getByRole('button',{name:'線路種別を適用'}));});
 await act(async()=>{fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});});
 expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='undo_world_edit'||command==='undo_alignment_deletion')).toBe(false);
 await act(async()=>{fireEvent.keyDown(document.body,{key:'z',ctrlKey:true,shiftKey:true});});
 expect(vi.mocked(invoke).mock.calls.filter(([c])=>c==='update_alignment_section_track_type')).toHaveLength(1);
 fireEvent.click(screen.getByRole('button',{name:'保存'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('save_world',expect.objectContaining({constructionPlans:expect.objectContaining({draftExtension:remappedEndpoint,parkedPaths:expect.arrayContaining([expect.objectContaining({destination:remappedEndpoint,redoDestination:remappedEndpoint})])})})));
});
