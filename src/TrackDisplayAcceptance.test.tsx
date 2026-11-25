// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
import {panMap} from './mapTestNavigation';
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

test('built centerlines and hit targets follow view changes without pruning world content',async()=>{
 const p=(longitude:number,latitude=35.67504)=>({latitude,longitude});
 const lines=[
  {id:'local',points:[p(139.76),p(139.77)]},
  {id:'crossing',points:[p(139.51),p(140.25)]},
  {id:'new-area',points:[p(139.99),p(140)]},
  ...Array.from({length:600},(_,i)=>({id:'distant-'+i,points:[p(141.3,43.1),p(141.31,43.1)]})),
 ].map(x=>({...x,name:x.id,owner:'2',track_type:'rail'}));
 const world={next_id:'999',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[],tracks:[],routes:[],services:[],alignments:lines,station_placements:[]};
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'track-viewport-test'};
  if(command==='list_save_slots')return {active_slot_id:'test',slots:[]};
  if(command==='autosave_status')return {available:false};
  if(command==='water_mask_requests')return [];
  return world;
 });
 render(<App/>);await screen.findByText(/track-viewport-test/);
 const map=screen.getByRole('img',{name:'駅配置用OpenStreetMap'});
 const ids=()=>Array.from(map.querySelectorAll('.route-base')).map(el=>el.getAttribute('data-alignment-id'));
 expect(ids()).toEqual(['local','crossing']);
 expect(map.querySelectorAll('.alignment-hit')).toHaveLength(2);
 panMap(730);
 await waitFor(()=>expect(ids()).toEqual(['crossing','new-area']));
 expect(map.querySelectorAll('.alignment-hit')).toHaveLength(2);
 expect(world.alignments).toHaveLength(603);
});
