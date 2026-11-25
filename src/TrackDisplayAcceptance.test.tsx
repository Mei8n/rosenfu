// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
import {panMap} from './mapTestNavigation';
HTMLDialogElement.prototype.showModal=function(){this.setAttribute('open','');};
HTMLDialogElement.prototype.close=function(){this.removeAttribute('open');};
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();localStorage.clear();});
test('built crossings remain assessed but no map marker or internal line directory appears after Escape',async()=>{
 const points=[{latitude:35.67,longitude:139.75},{latitude:35.68,longitude:139.76}];
 const world={next_id:'4',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[],tracks:[],routes:[],services:[],alignments:[{id:'3',owner:'2',name:'新しい線路',points}],station_placements:[],road_snapshots:[{source_url:'fixture'}]};
 vi.mocked(isTauri).mockReturnValue(true);vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'display-test'};
  if(command==='list_save_slots')return {active_slot_id:'test',slots:[]};
  if(command==='autosave_status')return {available:false};
  if(command==='road_assessments')return [['3',{crossings:[{road_id:'private-id',point:points[0],position:0}],roads_ready:true,unknown_road_height:false}]];
  if(command==='water_mask_requests')return [];
  return world;
 });
 render(<App/>);await screen.findByText(/display-test/);await waitFor(()=>expect(invoke).toHaveBeenCalledWith('road_assessments'));
 fireEvent.keyDown(window,{key:'Escape'});
 const map=screen.getByRole('img',{name:'駅配置用OpenStreetMap'});
 expect(map.querySelector('[data-crossing-alignment]')).toBeNull();expect(map.querySelector('.automatic-road-crossings')).toBeNull();
 expect(document.querySelector('.construction-list')).toBeNull();expect(screen.queryByRole('button',{name:'名前未設定の線路'})).toBeNull();
 expect(map.querySelector('.alignment-hit')).toBeTruthy();expect(document.body.textContent).not.toContain('private-id');
});

test('built and deletion hit paths follow view changes without renumbering sections',async()=>{
 const p=(longitude:number,latitude=35.67504)=>({latitude,longitude});
 const lines=[
  {id:'local',points:[p(139.76),p(139.77)]},
  {id:'crossing',points:[p(139.51),p(140.25)]},
  {id:'new-area',points:[p(140.39),p(140.27),p(139.99)],anchor_indices:[0,1,2],infrastructure:[{level:'ground',track_count:1},{level:'ground',track_count:2}]},
  ...Array.from({length:600},(_,i)=>({id:'distant-'+i,points:[p(141.3,43.1),p(141.31,43.1)]})),
 ].map(x=>({...x,name:x.id,owner:'2',track_type:'rail'}));
 const world={next_id:'999',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[],tracks:[],routes:[],services:[],alignments:lines,station_placements:[]};
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'track-viewport-test'};
  if(command==='list_save_slots')return {active_slot_id:'test',slots:[]};
  if(command==='autosave_status')return {available:false};
  if(command==='water_mask_requests')return [];
  if(command==='preview_track_render_geometry')return {lanes:lines.map(line=>({alignment_id:line.id,section_index:0,lane_index:0,points:line.points})),movements:[]};
  return world;
 });
 render(<App/>);await screen.findByText(/track-viewport-test/);
 const map=screen.getByRole('img',{name:'駅配置用OpenStreetMap'});
 const deleteIds=()=>Array.from(map.querySelectorAll('[data-delete-alignment]'),el=>el.getAttribute('data-delete-alignment')+':'+el.getAttribute('data-delete-section'));
 const ids=()=>Array.from(map.querySelectorAll('.route-base')).map(el=>el.getAttribute('data-alignment-id'));
 await waitFor(()=>expect([...map.querySelectorAll('[data-physical-alignment]')].map(el=>el.getAttribute('data-physical-alignment'))).toEqual(['local','crossing']));
 expect(ids()).toEqual(['local','crossing']);
 expect(map.querySelectorAll('.alignment-hit')).toHaveLength(2);
 fireEvent.click(screen.getByRole('tab',{name:'削除'}));
 expect(deleteIds()).toEqual(['local:0','crossing:0']);
 panMap(730);
 await waitFor(()=>expect(ids()).toEqual(['crossing','new-area']));
 expect(map.querySelectorAll('.alignment-hit')).toHaveLength(2);
 expect(deleteIds()).toEqual(['crossing:0','new-area:1']);
 expect([...map.querySelectorAll('[data-physical-alignment]')].map(el=>el.getAttribute('data-physical-alignment'))).toEqual(['crossing','new-area']);
 expect(world.alignments).toHaveLength(603);
 const target=map.querySelector('[data-delete-alignment="new-area"]')!;
 map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:1000,bottom:600,width:1000,height:600,toJSON:()=>({})});
 for(const type of ['pointerdown','pointerup']){
  const pointer=new Event(type,{bubbles:true,cancelable:true});
  Object.assign(pointer,{clientX:500,clientY:300,button:0,pointerId:100,isPrimary:true});
  fireEvent(target,pointer);
 }
 fireEvent.click(target);
 expect(invoke).not.toHaveBeenCalledWith('delete_alignment_section',expect.anything());
 fireEvent.click(screen.getByRole('button',{name:'削除する'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('delete_alignment_section',expect.objectContaining({alignmentId:'new-area',sectionIndex:1})));
});

test('junction selection follows the map dismissal controls but survives a drag',async()=>{
 const point={latitude:35.67504,longitude:139.75};
 const alignments=['幹線','本線','支線'].map((name,i)=>({id:String(i+3),name,owner:'2',track_type:'rail',points:[point,{...point,longitude:139.76,latitude:35.68+i*.01}]}));
 const port=(alignment_id:string)=>({alignment_id,endpoint:'start'});
 const world={next_id:'6',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[],tracks:[],routes:[],services:[],alignments,alignment_connections:[{a:port('3'),b:port('4')},{a:port('3'),b:port('5')}],station_placements:[]};
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'junction-dismissal-test'};
  if(command==='list_save_slots')return {active_slot_id:'test',slots:[]};
  if(command==='autosave_status')return {available:false};
  if(command==='water_mask_requests'||command==='road_assessments')return [];
  return world;
 });
 render(<App/>);await screen.findByText(/junction-dismissal-test/);
 fireEvent.keyDown(window,{key:"Escape"});
 const map=screen.getByRole('img',{name:'駅配置用OpenStreetMap'});
 const marker=()=>screen.getByRole('button',{name:/分岐ポイント：.*幹線/});
 const hasSelection=()=>marker().getAttribute('aria-pressed')==='true';
 fireEvent.keyDown(marker(),{key:'Enter'});
 expect(hasSelection()).toBe(true);
 fireEvent.keyDown(window,{key:'Escape',isComposing:true,keyCode:229});
 expect(hasSelection()).toBe(true);
 fireEvent.keyDown(window,{key:'Escape'});
 expect(hasSelection()).toBe(false);
 fireEvent.keyDown(marker(),{key:'Enter'});
 panMap(22);
 fireEvent.click(marker());
 expect(hasSelection()).toBe(true);
 fireEvent.click(map);
 expect(hasSelection()).toBe(true);
 map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:1000,bottom:600,width:1000,height:600,toJSON:()=>({})});
 const pointer=(type:string)=>{const e=new Event(type,{bubbles:true,cancelable:true});Object.assign(e,{clientX:500,clientY:300,button:0,pointerId:105,isPrimary:true});fireEvent(map,e);};
 pointer('pointerdown');pointer('pointerup');
 fireEvent.click(map);
 expect(hasSelection()).toBe(false);
});
