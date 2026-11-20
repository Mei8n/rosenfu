// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
import {stationGroupViews} from './stationGroups';
import {constructionPlanFingerprint,emptyConstructionPlans} from './constructionPlans';
import {plannedGeometry} from './editedGeometry';
import {panMap} from './mapTestNavigation';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
vi.mock('./stationGroups',{spy:true});
vi.mock('./constructionPlans',{spy:true});
vi.mock('./editedGeometry',{spy:true});
afterEach(()=>{cleanup();localStorage.clear();vi.clearAllMocks();});

test('panel input reuses unchanged station groups and construction plan comparisons',async()=>{
 const points=Array.from({length:64},(_,i)=>({latitude:35.67504,longitude:139.7+i*.0001}));
 const plans=emptyConstructionPlans();
 plans.parkedPaths=Array.from({length:4},(_,i)=>({id:i+2,planId:1,name:'',points:[points[0],points.at(-1)!],line:points,editedGeometry:{version:1,anchors:[points[0],points.at(-1)!],line:points},redo:[],extension:null,trackType:'normal',designSpeedKmh:120,roadFollowing:false}));
 const world={next_id:'1000',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'会社'}],routes:[],services:[],construction_plans:plans,
  alignments:[{id:'3',owner:'2',name:'線路',points}],
  stations:Array.from({length:20},(_,i)=>({id:String(i+10),owner:'2',name:`駅${i}`,latitude:35.67504,longitude:139.7+i*.0002})),
  station_placements:Array.from({length:20},(_,i)=>({station_id:String(i+10),alignment_id:'3',position:(i+1)/22,length_m:20,width_m:12}))};
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async command=>{
  if(command==='engine_info')return {version:'render-performance-test'};
  if(command==='world_snapshot')return world;
  if(command==='list_save_slots')return {active_slot_id:null,slots:[]};
  throw Error('unexpected '+command);
 });
 render(<App/>);
 await screen.findByText(/render-performance-test/);
 fireEvent.click(screen.getByRole('tab',{name:'設定'}));
 const input=screen.getByLabelText('会社名');
 vi.mocked(stationGroupViews).mockClear();
 vi.mocked(constructionPlanFingerprint).mockClear();
 vi.mocked(plannedGeometry).mockClear();
 for(let i=0;i<10;i++)fireEvent.change(input,{target:{value:`会社${i}`}});
 expect(input).toHaveProperty('value','会社9');
 panMap(40,20);
 expect(stationGroupViews).not.toHaveBeenCalled();
 expect(constructionPlanFingerprint).not.toHaveBeenCalled();
 expect(plannedGeometry).not.toHaveBeenCalled();
 // Actual plan edits must still invalidate the comparison used by save/undo.
 fireEvent.change(screen.getByLabelText('計画名（任意）'),{target:{value:'変更した計画'}});
 expect(constructionPlanFingerprint).toHaveBeenCalled();
 expect(vi.mocked(constructionPlanFingerprint).mock.calls.at(-1)?.[0].plans[0].name).toBe('変更した計画');
});
