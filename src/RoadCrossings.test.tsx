// @vitest-environment jsdom
import {act,cleanup,render,screen} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {useRoadAssessments} from './RoadCrossings';
import type {SavedRoadSnapshot,RoadAssessments} from './constructionRoads';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
const value:RoadAssessments=[['3',{crossings:[{road_id:'way/1',point:{latitude:35.67,longitude:139.75},position:.5}],roads_ready:true,unknown_road_height:false}]];
const first={road_snapshots:[{} as SavedRoadSnapshot]},second={road_snapshots:[{} as SavedRoadSnapshot]};
function Harness({world}:{world:typeof first}){const state=useRoadAssessments(world,true);return <><span>{state.pending?'pending':state.error?'failed':'ready'}</span><output data-testid="assessment">{JSON.stringify(state.value)}</output></>;}
test('edit or Undo removes stale assessments immediately and ignores late old-world IPC',async()=>{
 let finish!:(value:RoadAssessments)=>void;
 vi.mocked(invoke).mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve as typeof finish;})).mockResolvedValueOnce([]);
 const {rerender,container}=render(<Harness world={first}/>);rerender(<Harness world={second}/>);
 await screen.findByText('ready');await act(async()=>finish(value));expect(JSON.parse(screen.getByTestId('assessment').textContent!).flatMap((row:any)=>row[1].crossings)).toHaveLength(0);
 vi.mocked(invoke).mockResolvedValueOnce(value);rerender(<Harness world={first}/>);await screen.findByText('ready');expect(JSON.parse(screen.getByTestId('assessment').textContent!).flatMap((row:any)=>row[1].crossings)).toHaveLength(1);
 rerender(<Harness world={{road_snapshots:[]}}/>);expect(JSON.parse(screen.getByTestId('assessment').textContent!).flatMap((row:any)=>row[1].crossings)).toHaveLength(0);
});
test('failed assessment never invents crossings',async()=>{
 vi.mocked(invoke).mockRejectedValue(Error('unavailable'));const {container}=render(<Harness world={first}/>);await screen.findByText('failed');expect(JSON.parse(screen.getByTestId('assessment').textContent!).flatMap((row:any)=>row[1].crossings)).toHaveLength(0);
});
test('canonical boundary fragment changes replace the assessment and Undo restores its location',async()=>{
 const crossing=value[0][1].crossings[0];
 const before:RoadAssessments=[['3',{...value[0][1],roads_ready:false,crossings:[{...crossing,road_id:'ofm:east',point:{longitude:139.76806640625,latitude:35.6853489711398}}]}]];
 const after:RoadAssessments=[['3',{...before[0][1],crossings:[{...crossing,road_id:'ofm:west',point:{longitude:139.76806640625,latitude:35.68534327496442}}]}]];
 vi.mocked(invoke).mockResolvedValueOnce(before).mockResolvedValueOnce(after).mockResolvedValueOnce(before);
 const {container,rerender}=render(<Harness world={first}/>);await screen.findByText('ready');
 const marker=()=>JSON.parse(screen.getByTestId('assessment').textContent!).flatMap((row:any)=>row[1].crossings)[0];const initial=JSON.stringify(marker().point);
 rerender(<Harness world={second}/>);expect(marker()).toBeUndefined();await screen.findByText('ready');
 expect(JSON.parse(screen.getByTestId('assessment').textContent!).flatMap((row:any)=>row[1].crossings)).toHaveLength(1);expect(JSON.stringify(marker().point)).not.toBe(initial);
 rerender(<Harness world={first}/>);expect(marker()).toBeUndefined();await screen.findByText('ready');
 expect(JSON.parse(screen.getByTestId('assessment').textContent!).flatMap((row:any)=>row[1].crossings)).toHaveLength(1);expect(JSON.stringify(marker().point)).toBe(initial);
 expect(marker().road_id).toBe('ofm:east');
});
