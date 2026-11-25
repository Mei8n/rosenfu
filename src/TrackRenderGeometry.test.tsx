// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,render,screen,waitFor} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {TrackGeometryLayer,useTrackRenderGeometry,type TrackRenderGeometry} from './TrackRenderGeometry';
import {DEFAULT_CAMERA} from './map';
import {createTrackViewportIndex,intersectsTrackView,trackBounds,trackViewport} from './trackViewport';
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
function Harness({world,epoch=0,enabled=true}:{world:object;epoch?:number;enabled?:boolean}){
 const state=useTrackRenderGeometry(world,enabled,epoch);
 return <div>{state.value?'ready':state.error??'pending'}</div>;
}
function deferred<T>(){
 let resolve!:(value:T)=>void,reject!:(error:Error)=>void;
 const promise=new Promise<T>((ok,fail)=>{resolve=ok;reject=fail;});
 return {promise,resolve,reject};
}
test('does not accept late geometry from the previous world and clears old geometry immediately',async()=>{
 let resolveOld!:(x:TrackRenderGeometry)=>void;const first={},waiting={},second={};
 vi.mocked(invoke).mockImplementation((_command,args)=>(args as {world?:unknown})?.world===first?new Promise(resolve=>{resolveOld=resolve as typeof resolveOld;}):Promise.resolve(geometry));
 const {rerender}=render(<Harness world={first}/>);
 rerender(<Harness world={waiting}/>);
 expect(invoke).toHaveBeenCalledTimes(1);
 rerender(<Harness world={second} epoch={1}/>);
 await screen.findByText('ready');
 await act(async()=>{resolveOld({...geometry,lanes:[]});});expect(screen.getByText('ready')).toBeTruthy();
 expect(invoke).toHaveBeenLastCalledWith('preview_track_render_geometry',{world:second});expect(invoke).toHaveBeenCalledTimes(2);
});
test('reports invalid or failed results instead of inventing lane geometry',async()=>{
 vi.mocked(invoke).mockResolvedValue({});render(<Harness world={{}}/>);await waitFor(()=>expect(screen.getByText(/描画データが不正/)).toBeTruthy());
});
test('coalesces rapid edits to the newest pending snapshot without publishing a stale error',async()=>{
 const first={draft:1},middle={draft:2},last={draft:3};
 const active=deferred<TrackRenderGeometry>(),next=deferred<TrackRenderGeometry>();
 vi.mocked(invoke).mockReturnValueOnce(active.promise).mockReturnValueOnce(next.promise);
 const {rerender}=render(<Harness world={first}/>);
 rerender(<Harness world={middle}/>);
 rerender(<Harness world={last}/>);
 expect(invoke).toHaveBeenCalledTimes(1);
 expect(screen.getByText('pending')).toBeTruthy();
 await act(async()=>active.reject(Error('outdated failure')));
 expect(screen.getByText('pending')).toBeTruthy();
 expect(invoke).toHaveBeenCalledTimes(2);
 expect(invoke).toHaveBeenLastCalledWith('preview_track_render_geometry',{world:last});
 await act(async()=>next.resolve(geometry));
 expect(screen.getByText('ready')).toBeTruthy();
});
test('disabling or unmounting drops waiting geometry but enabling can try again',async()=>{
 const first={},second={},third={};
 const active=deferred<TrackRenderGeometry>(),next=deferred<TrackRenderGeometry>();
 vi.mocked(invoke).mockReturnValueOnce(active.promise).mockReturnValueOnce(next.promise);
 const {rerender,unmount}=render(<Harness world={first}/>);
 rerender(<Harness world={second}/>);
 rerender(<Harness world={second} enabled={false}/>);
 await act(async()=>active.resolve(geometry));
 expect(screen.getByText('pending')).toBeTruthy();
 expect(invoke).toHaveBeenCalledTimes(1);
 rerender(<Harness world={second}/>);
 expect(invoke).toHaveBeenLastCalledWith('preview_track_render_geometry',{world:second});
 rerender(<Harness world={third}/>);
 unmount();
 await act(async()=>next.resolve(geometry));
 expect(invoke).toHaveBeenCalledTimes(2);
});
test('explicit internal movements are rendered once rather than invented from crossing pixels',()=>{
 const {container}=render(<svg><TrackGeometryLayer geometry={{...geometry,movements:[],internal_movements:[{points:[b,c]},{points:[c,b]}]}} camera={DEFAULT_CAMERA} lines={[]} selectedId=""/></svg>);expect(container.querySelectorAll('[data-internal-track-movement]')).toHaveLength(1);expect(container.querySelectorAll('[data-track-movement]')).toHaveLength(0);
});

test('culls only offscreen native geometry and retains crossing lines and stroke margin',()=>{
 const make=(alignment_id:string,points:TrackRenderGeometry['lanes'][number]['points'])=>({alignment_id,section_index:0,lane_index:0,points});
 const p=(longitude:number)=>({...a,longitude});
 const strokes={...geometry,lanes:[
  make('visible',[a,b]),make('margin',[p(139.81),p(139.811)]),
  make('through',[p(139.60),p(139.90)]),make('elsewhere',[p(140.0),p(140.1)]),
 ],movements:[geometry.movements[0],geometry.movements[1],{...geometry.movements[0],points:[p(140),p(140.1)]}]};
 const mapSize={width:500,height:600};
 const {container,rerender}=render(<svg><TrackGeometryLayer geometry={strokes} camera={DEFAULT_CAMERA} mapSize={mapSize} lines={[]} selectedId=""/></svg>);
 const lanes=()=>[...container.querySelectorAll('[data-physical-alignment]')].map(e=>e.getAttribute('data-physical-alignment'));
 expect(lanes()).toEqual(['visible','margin','through']);
 expect(container.querySelectorAll('[data-track-movement]')).toHaveLength(1);
 rerender(<svg><TrackGeometryLayer geometry={strokes} camera={{...DEFAULT_CAMERA,longitude:140.05}} mapSize={mapSize} lines={[]} selectedId="elsewhere"/></svg>);
 expect(lanes()).toEqual(['elsewhere']);expect(container.querySelector('[data-physical-alignment]')?.classList.contains('alignment-selected')).toBe(true);
 expect(container.querySelector('[data-track-movement]')?.getAttribute('data-track-movement')).toBe('1');
});

test('large dispersed map indexes infrastructure once and mounts only visible native lanes',()=>{
 let reads=0;
 const lines=Array.from({length:6000},(_,index)=>({get id(){reads++;return String(index);}}));
 const lanes=lines.map((_,index)=>({alignment_id:String(index),section_index:0,lane_index:0,points:index<12?[a,b]:[{latitude:43.1,longitude:141.3},{latitude:43.2,longitude:141.4}]}));
 const large={lanes,movements:[]};
 const {container,rerender}=render(<svg><TrackGeometryLayer geometry={large} camera={DEFAULT_CAMERA} lines={lines} selectedId=""/></svg>);
 expect(container.querySelectorAll('[data-physical-alignment]')).toHaveLength(12);
 expect(reads).toBe(6000);
 rerender(<svg><TrackGeometryLayer geometry={large} camera={{...DEFAULT_CAMERA,longitude:139.755}} lines={lines} selectedId=""/></svg>);
 expect(container.querySelectorAll('[data-physical-alignment]')).toHaveLength(12);
 expect(reads).toBe(6000);
});


test('indexed viewport matches exhaustive search at cell boundaries and broad zooms',()=>{
 const point=(longitude:number,latitude:number)=>({longitude,latitude});
 const rows=Array.from({length:600},(_,id)=>{
  const longitude=127+(id%42)*.3,latitude=30+(id%25)*.48;
  return {id,points:[point(longitude,latitude),point(longitude+.13,latitude+.11)]};
 });
 rows.splice(30,0,
  {id:901,points:[point(137,35.75),point(143,35.75)]},
  {id:902,points:[point(139.749,35.749),point(139.75,35.75)]},
  {id:903,points:[point(139.75,35.75),point(139.751,35.751)]},
  {id:904,points:[]},
 );
 const indexed=createTrackViewportIndex(rows);
 const bounds=[
  {west:139.75,east:139.7501,south:35.75,north:35.7501},
  trackViewport({...DEFAULT_CAMERA,zoom:4},{width:1000,height:600}),
  ...Array.from({length:24},(_,i)=>trackViewport({latitude:30+i*.48,longitude:139-i*.31,zoom:12},{width:750,height:400})),
 ];
 for(const view of bounds)expect(indexed.visible(view)).toEqual(rows.filter(row=>intersectsTrackView(trackBounds(row.points),view)));
 expect(indexed.visible(bounds[0]).map(row=>row.id)).toEqual([901,902,903]);
});
