// @vitest-environment jsdom
import {act,cleanup,render,screen} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {RoadCrossings,useRoadAssessments} from './RoadCrossings';
import type {SavedRoadSnapshot,RoadAssessments} from './constructionRoads';
import {DEFAULT_CAMERA} from './map';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
const value:RoadAssessments=[['3',{crossings:[{road_id:'way/1',point:{latitude:35.67,longitude:139.75},position:.5}],roads_ready:true,unknown_road_height:false}]];
const first={road_snapshots:[{} as SavedRoadSnapshot]},second={road_snapshots:[{} as SavedRoadSnapshot]};
function Harness({world}:{world:typeof first}){const state=useRoadAssessments(world,true);return <><span>{state.pending?'pending':state.error?'failed':'ready'}</span><svg><RoadCrossings assessments={state.value} camera={DEFAULT_CAMERA}/></svg></>;}
test('edit or Undo removes stale markers immediately and ignores late old-world IPC',async()=>{
 let finish!:(value:RoadAssessments)=>void;
 vi.mocked(invoke).mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve as typeof finish;})).mockResolvedValueOnce([]);
 const {rerender,container}=render(<Harness world={first}/>);rerender(<Harness world={second}/>);
 await screen.findByText('ready');await act(async()=>finish(value));expect(container.querySelectorAll('[data-crossing-alignment]')).toHaveLength(0);
 vi.mocked(invoke).mockResolvedValueOnce(value);rerender(<Harness world={first}/>);await screen.findByText('ready');expect(container.querySelectorAll('[data-crossing-alignment]')).toHaveLength(1);
 rerender(<Harness world={{road_snapshots:[]}}/>);expect(container.querySelectorAll('[data-crossing-alignment]')).toHaveLength(0);
});
test('failed assessment never invents crossing markers',async()=>{
 vi.mocked(invoke).mockRejectedValue(Error('unavailable'));const {container}=render(<Harness world={first}/>);await screen.findByText('failed');expect(container.querySelectorAll('[data-crossing-alignment]')).toHaveLength(0);
});
