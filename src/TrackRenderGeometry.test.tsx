// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,render,screen,waitFor} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {TrackGeometryLayer,useTrackRenderGeometry,type TrackRenderGeometry} from './TrackRenderGeometry';
import {DEFAULT_CAMERA} from './map';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
const a={latitude:35.67504,longitude:139.75},b={...a,longitude:139.7501},c={...a,longitude:139.7502};
const from={endpoint:{alignment_id:'1',endpoint:'end' as const},lane_index:0},to={endpoint:{alignment_id:'2',endpoint:'start' as const},lane_index:0};
const geometry:TrackRenderGeometry={lanes:[{alignment_id:'1',section_index:0,lane_index:0,points:[a,b]}],movements:[{movement:{from,to,point:{}},from_position:.9,to_position:.1,points:[b,c]},{movement:{from:to,to:from,point:{}},from_position:.1,to_position:.9,points:[c,b]}]};
test('uses physical shared joins and renders only one direction at every zoom',()=>{
 const {container,rerender}=render(<svg><TrackGeometryLayer geometry={geometry} camera={DEFAULT_CAMERA} lines={[]} selectedId=""/></svg>);
 for(const zoom of [12,16,20]){rerender(<svg><TrackGeometryLayer geometry={geometry} camera={{...DEFAULT_CAMERA,zoom}} lines={[]} selectedId=""/></svg>);
 const lane=container.querySelector('[data-physical-alignment]')!.getAttribute('d')!,movement=container.querySelector('[data-track-movement]')!.getAttribute('d')!;
 expect(container.querySelectorAll('[data-track-movement]')).toHaveLength(1);expect(movement.split(' ')[0].slice(1)).toBe(lane.split(' ').at(-1)!.slice(1));}
});
function Harness({world}:{world:object}){const state=useTrackRenderGeometry(world,true);return <div>{state.value?'ready':state.error?'failed':'pending'}</div>;}
test('does not accept late geometry from the previous world and clears old geometry immediately',async()=>{
 let resolveOld!:(x:TrackRenderGeometry)=>void;const first={},second={};
 vi.mocked(invoke).mockImplementation((_command,args)=>(args as {world?:unknown})?.world===first?new Promise(resolve=>{resolveOld=resolve as typeof resolveOld;}):Promise.resolve(geometry));
 const {rerender}=render(<Harness world={first}/>);rerender(<Harness world={second}/>);await screen.findByText('ready');
 await act(async()=>{resolveOld({...geometry,lanes:[]});});expect(screen.getByText('ready')).toBeTruthy();
 expect(invoke).toHaveBeenCalledWith('preview_track_render_geometry',{world:second});
});
test('reports invalid or failed results instead of inventing lane geometry',async()=>{
 vi.mocked(invoke).mockResolvedValue({});render(<Harness world={{}}/>);await waitFor(()=>expect(screen.getByText('failed')).toBeTruthy());
});
test('explicit internal movements are rendered once rather than invented from crossing pixels',()=>{
 const {container}=render(<svg><TrackGeometryLayer geometry={{...geometry,movements:[],internal_movements:[{points:[b,c]},{points:[c,b]}]}} camera={DEFAULT_CAMERA} lines={[]} selectedId=""/></svg>);expect(container.querySelectorAll('[data-internal-track-movement]')).toHaveLength(1);expect(container.querySelectorAll('[data-track-movement]')).toHaveLength(0);
});
