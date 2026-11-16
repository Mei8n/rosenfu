// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
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
