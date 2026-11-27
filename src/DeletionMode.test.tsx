// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
import {emptyConstructionPlans} from './constructionPlans';
import {anchorIndices} from './deletionSections';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
vi.mock('./deletionSections',async(importOriginal)=>{
 const original=await importOriginal<typeof import('./deletionSections')>();
 return {...original,anchorIndices:vi.fn(original.anchorIndices)};
});
const a={latitude:35.67504,longitude:139.72},b={latitude:35.67504,longitude:139.75},c={latitude:35.67504,longitude:139.78};
const empty={next_id:'10',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
const alignment={id:'3',owner:'2',name:'線路',points:[a,b,c],anchor_indices:[0,1,2]};
const built={...empty,alignments:[alignment]};
HTMLDialogElement.prototype.showModal=function(){this.setAttribute('open','');};
HTMLDialogElement.prototype.close=function(){this.removeAttribute('open');};
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();});
async function start(snapshot:unknown=built){
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async (command,args)=>{if(command==='engine_info')return {name:'Rosenfu',version:'mode-test'};if(command==='world_snapshot')return snapshot;if(command==='delete_alignment_section'||command==='delete_station')return empty;if(command==='undo_world_edit'||command==='redo_world_edit')return {...(command==='undo_world_edit'?snapshot as object:empty),construction_plans:(args as any).expectedWorld.construction_plans,view_state:(args as any).expectedWorld.view_state};throw Error(command);});
 render(<App/>);await screen.findByText(/mode-test/);
 const map=screen.getByRole('img',{name:'駅配置用OpenStreetMap'});map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 fireEvent.click(screen.getByRole('tab',{name:'削除'}));return map;
}
test('dedicated deletion mode highlights only selected real anchor interval, cancels then deletes and undoes',async()=>{
 const map=await start();const targets=map.querySelectorAll('[data-delete-alignment="3"]');expect(targets).toHaveLength(2);expect(targets[0].getAttribute("fill")).toBe("none");expect(targets[0].getAttribute("pointer-events")).toBe("stroke");
 fireEvent.pointerEnter(targets[1],{clientX:200,clientY:150});expect(targets[1].classList.contains('deletion-hover')).toBe(true);expect(targets[0].classList.contains('deletion-hover')).toBe(false);
 fireEvent.click(targets[1]);expect(screen.getByRole('dialog')).toBeTruthy();fireEvent.click(screen.getByRole('button',{name:'キャンセル'}));expect(invoke).not.toHaveBeenCalledWith('delete_alignment_section',expect.anything());
 fireEvent.click(targets[1]);expect(document.activeElement).toBe(screen.getByRole('button',{name:'削除する'}));await act(async()=>{fireEvent.keyDown(document.activeElement!,{key:'Enter'});});await waitFor(()=>expect(map.querySelector('[data-delete-alignment]')).toBeNull());
 expect(invoke).toHaveBeenCalledWith('delete_alignment_section',expect.objectContaining({alignmentId:'3',sectionIndex:1,constructionPlans:expect.any(Object)}));
 expect(screen.queryByRole('button',{name:'削除を元に戻す'})).toBeNull();
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});await waitFor(()=>expect(map.querySelectorAll('[data-delete-alignment]')).toHaveLength(2));
});
test('individual plan anchor deletion leaves a finished singleton and supports undo',async()=>{
 const map=await start({...empty,construction_plans:{...emptyConstructionPlans(),draftPoints:[a,b],draftFinished:true}});
 fireEvent.click(map.querySelector('[data-delete-anchor="1"]')!);expect(screen.queryByRole('dialog')).toBeNull();expect(map.querySelectorAll('[data-delete-anchor]')).toHaveLength(1);expect(map.querySelector('.candidate-draft')).toBeNull();
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelectorAll('[data-delete-anchor]')).toHaveLength(2);
 expect(vi.mocked(invoke).mock.calls.some(([cmd])=>cmd==='build_plan'||cmd==='delete_alignment_section')).toBe(false);
});
test('built stations are removable through the same map mode',async()=>{
 const map=await start({...empty,stations:[{id:'4',owner:'2',name:'駅',...b}]});fireEvent.click(map.querySelector('[data-delete-station="4"]')!);fireEvent.click(screen.getByRole('button',{name:'削除する'}));
 await waitFor(()=>expect(map.querySelector('[data-delete-station]')).toBeNull());expect(invoke).toHaveBeenCalledWith('delete_station',expect.objectContaining({stationId:'4',constructionPlans:expect.any(Object)}));
});
test('connected line section reaches backend instead of whole-line reference guard',async()=>{
 const snapshot={...built,alignment_connections:[{a:{alignment_id:'3',endpoint:'end'},b:{alignment_id:'5',endpoint:'start'}}]};
 const map=await start(snapshot);
 fireEvent.click(map.querySelector('[data-delete-section="0"]')!);
 expect((screen.getByRole('button',{name:'削除する'}) as HTMLButtonElement).disabled).toBe(false);
 fireEvent.click(screen.getByRole('button',{name:'削除する'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('delete_alignment_section',expect.objectContaining({alignmentId:'3',sectionIndex:0})));
});
test('station on surviving section is validated by backend instead of blocked globally',async()=>{
 const map=await start({...built,stations:[{id:'4',owner:'2',name:'駅',...c}],station_placements:[{station_id:'4',alignment_id:'3',position:.9,length_m:200,width_m:12}]});
 fireEvent.click(map.querySelector('[data-delete-section="0"]')!);
 fireEvent.click(screen.getByRole('button',{name:'削除する'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('delete_alignment_section',expect.objectContaining({alignmentId:'3',sectionIndex:0})));
});
test.each(['接続端点を含む区間は削除できません。','駅のホームに重なる区間は削除できません。','路線・系統が使用中の線路の区間削除には対応していません。'])('backend rejection stays atomic and can be cancelled: %s',async message=>{
 const map=await start();
 vi.mocked(invoke).mockRejectedValueOnce(message);
 fireEvent.click(map.querySelector('[data-delete-section="0"]')!);
 fireEvent.click(screen.getByRole('button',{name:'削除する'}));
 await screen.findByText(message,{exact:true});
 expect(map.querySelectorAll('[data-delete-alignment="3"]')).toHaveLength(2);
 expect(screen.queryByRole('button',{name:'削除を元に戻す'})).toBeNull();
 fireEvent.click(screen.getByRole('button',{name:'キャンセル'}));
 expect(screen.queryByRole('dialog')).toBeNull();
});
test('anchor rejection leaves plan intact without opening confirmation',async()=>{
 const map=await start({...empty,construction_plans:{...emptyConstructionPlans(),draftPoints:[a,b],draftFinished:true,roadFollowing:true}});
 fireEvent.click(map.querySelector('[data-delete-anchor="1"]')!);
 expect(screen.queryByRole('dialog')).toBeNull();
 expect(map.querySelectorAll('[data-delete-anchor]')).toHaveLength(2);
 expect(screen.getByText(/参照線路のアンカーは個別に削除できません/)).toBeTruthy();
 expect(screen.queryByRole('button',{name:'削除を元に戻す'})).toBeNull();
});
test('other company section remains blocked before invocation',async()=>{
 const map=await start({...built,alignments:[{...alignment,owner:'99'}]});
 fireEvent.click(map.querySelector('[data-delete-section="0"]')!);
 expect((screen.getByRole('button',{name:'削除する'}) as HTMLButtonElement).disabled).toBe(true);
 expect(invoke).not.toHaveBeenCalledWith('delete_alignment_section',expect.anything());
});

test('idle construction hides line name and internal identifier without changing selection',async()=>{
 const map=await start();
 fireEvent.click(screen.getByRole('button',{name:'選択'}));
 fireEvent.click(map.querySelector('.alignment-hit')!);
 expect(map.querySelector('[data-alignment-id="3"] title')).toBeNull();
 expect(screen.queryByText('名前未設定の線路')).toBeNull();
 expect(screen.getByRole('button',{name:'選択線路を削除'})).toBeTruthy();
 fireEvent.click(screen.getByRole('tab',{name:'削除'}));
 expect(map.querySelector('[data-alignment-id="3"] title')?.textContent).toBe('線路');
});
test('planned intervals highlight and remove independently, preserving both sides and undo/save payload',async()=>{
 const d={latitude:a.latitude,longitude:139.81};
 const original={...emptyConstructionPlans(),draftPoints:[a,b,c,d],draftFinished:true};
 const map=await start({...empty,construction_plans:original});
 const sections=map.querySelectorAll('[data-delete-plan-section]');expect(sections).toHaveLength(3);
 fireEvent.pointerEnter(sections[1],{clientX:200,clientY:150});expect(sections[1].classList.contains('deletion-hover')).toBe(true);expect(sections[0].classList.contains('deletion-hover')).toBe(false);
 fireEvent.click(sections[1]);expect(screen.getByRole('dialog').textContent).toContain('計画線路の区間');
 fireEvent.click(screen.getByRole('button',{name:'キャンセル'}));expect(map.querySelectorAll('[data-delete-plan-section]')).toHaveLength(3);
 fireEvent.click(sections[1]);fireEvent.click(screen.getByRole('button',{name:'削除する'}));
 expect(map.querySelectorAll('[data-delete-plan-section]')).toHaveLength(2);
 expect(map.querySelectorAll('[data-delete-plan-path="1"]')).toHaveLength(1);expect(map.querySelectorAll('[data-delete-plan-path="2"]')).toHaveLength(1);
 expect(map.querySelectorAll('[data-delete-anchor]')).toHaveLength(4);expect(map.querySelector('.candidate-draft')).toBeNull();
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelectorAll('[data-delete-plan-section]')).toHaveLength(3);expect(map.querySelector('[data-delete-plan-path="2"]')).toBeNull();
});
test('opening a hover on a long planned path does not remap its anchors once per section',async()=>{
 const anchors=Array.from({length:25},(_,i)=>({...a,longitude:a.longitude+i*.001}));
 const map=await start({...empty,construction_plans:{...emptyConstructionPlans(),draftPoints:anchors,draftFinished:true}});
 const sections=map.querySelectorAll('[data-delete-plan-path="1"]');
 expect(sections).toHaveLength(24);
 const indices=vi.mocked(anchorIndices);
 indices.mockClear();
 fireEvent.pointerEnter(sections[12],{clientX:200,clientY:150});
 expect(sections[12].classList.contains('deletion-hover')).toBe(true);
 // Construction preview also uses a mapping; no matter how many
 // sections are exposed there are only a handful of calls on this render.
 const calls=indices.mock.calls.filter(([points])=>points.length===anchors.length).length;
 expect(calls).toBeGreaterThan(0);
 expect(calls).toBeLessThanOrEqual(4);
});

test('whole-path-only road-follow plan stays whole and supports undo and redo',async()=>{
 const map=await start({...empty,construction_plans:{...emptyConstructionPlans(),draftPoints:[a,b,c],draftFinished:true,roadFollowing:true}});
 const whole=()=>map.querySelectorAll('[data-delete-plan-path="1"]');
 expect(whole()).toHaveLength(1);
 expect(whole()[0].getAttribute('d')?.match(/L/g)).toHaveLength(2);
 fireEvent.click(whole()[0]);
 expect(screen.getByRole('dialog').textContent).not.toContain('計画線路の区間');
 fireEvent.click(screen.getByRole('button',{name:'削除する'}));
 expect(whole()).toHaveLength(0);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 expect(whole()).toHaveLength(1);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true,shiftKey:true});
 expect(whole()).toHaveLength(0);
});
test('split connection saves, reloads and builds two correctly oriented extensions',async()=>{
 const d={latitude:a.latitude,longitude:139.81},west={latitude:a.latitude,longitude:139.69},east={latitude:a.latitude,longitude:139.84};
 const world={...empty,alignments:[{id:'3',owner:'2',name:'source',points:[west,a]},{id:'4',owner:'2',name:'destination',points:[d,east]}]};
 const plans={...emptyConstructionPlans(),draftPoints:[a,b,c,d],draftExtension:{alignmentId:'3',endpoint:'end' as const},draftDestination:{alignmentId:'4',endpoint:'start' as const},draftFinished:true,plannedStations:[{id:'planned:1',pathId:1,planId:1,name:'tail station',point:{latitude:a.latitude,longitude:139.7875},lengthM:100}]};
 const map=await start({...world,construction_plans:plans});
 fireEvent.click(map.querySelector('[data-delete-plan-section="1"]')!);fireEvent.click(screen.getByRole('button',{name:'削除する'}));
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 expect(map.querySelectorAll('[data-delete-plan-section]')).toHaveLength(3);
 expect(map.querySelector('[data-delete-plan-path="2"]')).toBeNull();
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true,shiftKey:true});
 expect(map.querySelectorAll('[data-delete-plan-section]')).toHaveLength(2);
 expect(map.querySelector('[data-delete-plan-path="2"]')).not.toBeNull();
 vi.mocked(invoke).mockResolvedValueOnce({saved:true});fireEvent.click(screen.getByRole('button',{name:'保存'}));
 await screen.findByText('ワールドと未着工計画を保存しました。');
 const saved=(vi.mocked(invoke).mock.calls.find(([c])=>c==='save_world')![1] as {constructionPlans:typeof plans}).constructionPlans;
 expect(saved.plannedStations[0].pathId).toBe(2);
 expect(saved.plannedStations[0].point).toEqual(plans.plannedStations[0].point);
 cleanup();vi.mocked(invoke).mockClear();
 const restored=await start({...world,construction_plans:JSON.parse(JSON.stringify(saved))});
 expect(restored.querySelectorAll('[data-delete-plan-section]')).toHaveLength(2);
 fireEvent.click(screen.getByRole('tab',{name:'線路'}));
 const build=screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement;expect(build.disabled).toBe(false);
 vi.mocked(invoke).mockResolvedValueOnce(world);fireEvent.click(build);
 await waitFor(()=>expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='build_plan')).toBe(true));
 const request=(vi.mocked(invoke).mock.calls.find(([c])=>c==='build_plan')![1] as {alignments:{points:unknown[];extension:unknown;destination?:unknown;stations:{position:number;lengthM:number;name:string}[]}[]}).alignments;
 expect(request).toHaveLength(2);expect(request.find(p=>(p.extension as any).alignmentId==='3')?.points).toEqual([a,b]);expect(request.find(p=>(p.extension as any).alignmentId==='4')?.points).toEqual([d,c]);expect(request.every(p=>!p.destination)).toBe(true);
 const station=request.find(p=>(p.extension as any).alignmentId==='4')!.stations[0];expect(station.name).toBe('tail station');expect(station.lengthM).toBe(100);expect(station.position).toBeCloseTo(.75,5);
});

test('planned path deletion supports repeated undo and redo',async()=>{
 const map=await start({...empty,construction_plans:{...emptyConstructionPlans(),draftPoints:[a,b],draftFinished:true}});
 fireEvent.click(map.querySelector('[data-delete-plan-path]')!);fireEvent.click(screen.getByRole('button',{name:'削除する'}));
 for(let i=0;i<2;i++){
  expect(map.querySelector('[data-delete-plan-path]')).toBeNull();
  fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelector('[data-delete-plan-path]')).not.toBeNull();
  fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});
 }
 expect(map.querySelector('[data-delete-plan-path]')).toBeNull();
});
test('built station deletion supports repeated undo and redo through backend validation',async()=>{
 const map=await start({...empty,stations:[{id:'4',owner:'2',name:'駅',...b}]});
 fireEvent.click(map.querySelector('[data-delete-station="4"]')!);fireEvent.click(screen.getByRole('button',{name:'削除する'}));
 await waitFor(()=>expect(map.querySelector('[data-delete-station]')).toBeNull());
 for(let i=0;i<2;i++){
  await waitFor(()=>expect((screen.getByRole('tab',{name:'削除'}) as HTMLButtonElement).disabled).toBe(false));
 expect(screen.queryByRole('button',{name:'削除を元に戻す'})).toBeNull();
  fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});await waitFor(()=>expect(map.querySelector('[data-delete-station]')).not.toBeNull());
  await waitFor(()=>expect((screen.getByRole('tab',{name:'削除'}) as HTMLButtonElement).disabled).toBe(false));
  fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});await waitFor(()=>expect(map.querySelector('[data-delete-station]')).toBeNull());
 }
 expect(vi.mocked(invoke).mock.calls.filter(([cmd])=>cmd==='delete_station')).toHaveLength(1);expect(vi.mocked(invoke).mock.calls.filter(([cmd])=>cmd==='redo_world_edit')).toHaveLength(2);
});

test('planned anchor deletion redo is discarded by a different deletion',async()=>{
 const map=await start({...empty,construction_plans:{...emptyConstructionPlans(),draftPoints:[a,b,c],draftFinished:true}});
 fireEvent.click(map.querySelector('[data-delete-anchor="1"]')!);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 expect(map.querySelectorAll('[data-delete-anchor]')).toHaveLength(3);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true,shiftKey:true});
 expect(map.querySelectorAll('[data-delete-anchor]')).toHaveLength(2);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 fireEvent.click(map.querySelector('[data-delete-anchor="0"]')!);
 const points=map.querySelector('[data-delete-plan-path]')?.getAttribute('d');
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true,shiftKey:true});
 expect(map.querySelector('[data-delete-plan-path]')?.getAttribute('d')).toBe(points);
});
test('redo section removal revalidates backend and leaves restored world intact on rejection',async()=>{
 const map=await start();
 fireEvent.click(map.querySelector('[data-delete-section="0"]')!);await act(async()=>{fireEvent.click(screen.getByRole('button',{name:'削除する'}));});
 await waitFor(()=>expect((screen.getByRole('tab',{name:'削除'}) as HTMLButtonElement).disabled).toBe(false));
 expect(screen.queryByRole('button',{name:'削除を元に戻す'})).toBeNull();
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 await waitFor(()=>expect(map.querySelectorAll('[data-delete-section]')).toHaveLength(2));
 await waitFor(()=>expect((screen.getByRole('tab',{name:'削除'}) as HTMLButtonElement).disabled).toBe(false));
 vi.mocked(invoke).mockRejectedValueOnce('駅のホームに重なる区間は削除できません。');
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true,shiftKey:true});
 await screen.findByText(/駅のホームに重なる区間は削除できません。/);
 expect(map.querySelectorAll('[data-delete-section]')).toHaveLength(2);
});

test('planned station deletion can be undone and redone without changing its track',async()=>{
 const map=await start({...empty,construction_plans:{...emptyConstructionPlans(),draftPoints:[a,b,c],draftFinished:true,plannedStations:[{id:'planned:1',pathId:1,planId:1,name:'planned station',point:b,lengthM:100}]}});
 fireEvent.click(map.querySelector('[data-delete-planned-station]')!);fireEvent.click(screen.getByRole('button',{name:'削除する'}));
 expect(map.querySelector('[data-delete-planned-station]')).toBeNull();
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelector('[data-delete-planned-station]')).not.toBeNull();
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true,shiftKey:true});expect(map.querySelector('[data-delete-planned-station]')).toBeNull();
 expect(map.querySelectorAll('[data-delete-anchor]')).toHaveLength(3);
});
test('changing a restored plan invalidates deletion redo even if the setting is changed back',async()=>{
 const map=await start({...empty,construction_plans:{...emptyConstructionPlans(),draftPoints:[a,b],draftFinished:true}});
 fireEvent.click(map.querySelector('[data-delete-plan-path]')!);fireEvent.click(screen.getByRole('button',{name:'削除する'}));
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 fireEvent.click(screen.getByRole('tab',{name:'線路'}));
 const speed=screen.getByRole('spinbutton',{name:'線路の目標最高速度'});
 fireEvent.change(speed,{target:{value:'160'}});fireEvent.change(speed,{target:{value:'120'}});
 fireEvent.click(screen.getByRole('tab',{name:'削除'}));
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true,shiftKey:true});
 expect(map.querySelector('[data-delete-plan-path]')).not.toBeNull();
});

test.each(['built', 'planned'] as const)('%s section confirmation defaults to deletion, while Escape cancels',async kind=>{
 const map=await start(kind==='built'?built:{...empty,construction_plans:{...emptyConstructionPlans(),draftPoints:[a,b,c],draftFinished:true}});
 const selector=kind==='built'?'[data-delete-section="0"]':'[data-delete-plan-section="0"]';
 fireEvent.click(map.querySelector(selector)!);
 expect(document.activeElement).toBe(screen.getByRole('button',{name:'削除する'}));
 fireEvent(screen.getByRole('dialog'),new Event('cancel',{cancelable:true}));
 expect(screen.queryByRole('dialog')).toBeNull();
 expect(map.querySelectorAll(selector)).toHaveLength(1);
 expect(invoke).not.toHaveBeenCalledWith('delete_alignment_section',expect.anything());
 fireEvent.click(map.querySelector(selector)!);
 expect(document.activeElement).toBe(screen.getByRole('button',{name:'削除する'}));
 await act(async()=>{fireEvent.keyDown(document.activeElement!,{key:'Enter'});});
 expect(screen.queryByRole('dialog')).toBeNull();
 if(kind==='built')expect(map.querySelector('[data-delete-section]')).toBeNull();
 else expect(map.querySelectorAll('[data-delete-plan-section]')).toHaveLength(1);
});

test('parked plans cull only map targets, keeping crossing lines, lone anchors and original section IDs',async()=>{
 const near=(longitude:number)=>({...b,longitude});
 const west=near(138.8),west2=near(139.2),east=near(140.2);
 const points=[west,west2,east],solo=[near(139.76)],off=[near(138.7),near(138.75)];
 const make=(id:number,anchors:typeof points,line=anchors)=>({
  id,planId:id,name:'計画 '+id,points:anchors,line,redo:[],designSpeedKmh:120,
  trackType:'normal' as const,roadFollowing:false,extension:null,
 });
 const construction_plans={...emptyConstructionPlans(),draftPoints:[a],plans:[1,2,3,4].map(id=>({id,name:'計画 '+id})),
  parkedPaths:[make(2,points),make(3,solo,[]),make(4,off)]};
 const map=await start({...empty,construction_plans});
 expect(map.querySelector('[data-plan-id="2"]')?.closest('.inactive-plan')).not.toBeNull();
 expect(map.querySelector('[data-delete-plan-path="2"][data-delete-plan-section="1"]')).not.toBeNull();
 expect(map.querySelector('[data-delete-path="3"][data-delete-anchor="0"]')).not.toBeNull();
 expect(map.querySelector('[data-plan-id="4"],[data-delete-plan-path="4"],[data-delete-path="4"]')).toBeNull();
 fireEvent.click(screen.getByRole('button',{name:'選択'}));
 fireEvent.click(map.querySelector('[data-plan-hover-id="2"]')!);
 expect(map.querySelector('[data-plan-id="2"]')?.classList.contains('track-explicit-selection')).toBe(true);
 expect(map.querySelector('.candidate-draft')).toBeNull();
 // Return to the world on disk with the camera pointing at plan 4. Its
 // geometry and deletion identities are still available, not discarded.
 cleanup();
 const other=await start({...empty,construction_plans,view_state:{latitude:b.latitude,longitude:138.72,zoom:12}});
 expect(other.querySelector('[data-plan-id="4"]')).not.toBeNull();
 expect(other.querySelector('[data-delete-plan-path="4"][data-delete-plan-section="0"]')).not.toBeNull();
 expect(other.querySelector('[data-delete-path="3"]')).toBeNull();
});
