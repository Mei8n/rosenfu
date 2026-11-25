// @vitest-environment jsdom
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,render} from '@testing-library/react';
import {invoke} from '@tauri-apps/api/core';
import {OperationMapMarkers} from './OperationMapMarkers';
import {OperationProvider,OperationVehicleLayer,type OperationView} from './OperationsPanel';
import {project} from './map';

vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();vi.useRealTimers();});

const camera={latitude:35,longitude:139,zoom:12};
const marker=(position=.5)=>({key:{service_id:'9007199254740993',scheduled_ms:12},alignment_start:{latitude:35,longitude:138.999},alignment_end:{latitude:35,longitude:139.001},position});
const panel=({epoch=1,worldVersion='one'}:{epoch?:number;worldVersion?:string}={})=>
 <OperationProvider available blocked={false} epoch={epoch} worldVersion={worldVersion} onChange={()=>{}}>
  <svg><OperationVehicleLayer camera={camera} scale={2}/></svg>
 </OperationProvider>;
const snapshot:OperationView={
 now_ms:1,paused:false,speed:1,invalidated:false,catching_up:false,timetable_edit_token:'edit-1',
 configuration_token:'configuration-1',services:[],trains:[],rail_markers:[marker()],
};

test('passive markers use supplied positions only, without IDs or invented substitutes',()=>{
 const {container,rerender}=render(<svg><OperationMapMarkers markers={[
  marker(),marker(1),{...marker(),position:Infinity},{...marker(),alignment_start:{latitude:35,longitude:170},alignment_end:{latitude:35,longitude:171}},
 ]} camera={camera} scale={2}/></svg>);
 expect(container.querySelectorAll('circle')).toHaveLength(1);
 expect(container.querySelector('g')?.getAttribute('pointer-events')).toBe('none');
 expect(Number(container.querySelector('circle')?.getAttribute('cx'))).toBeCloseTo(500,6);
 expect(container.querySelector('circle')?.getAttribute('r')).toBe('2');
 expect(container.innerHTML).not.toContain('9007199254740993');
 rerender(<svg><OperationMapMarkers markers={undefined} camera={camera} scale={2}/></svg>);
 expect(container.querySelector('circle')).toBeNull();
});

test('moves only on the existing native operation tick and clears on world change',async()=>{
 vi.useFakeTimers();
 const {services:_,...compact}=snapshot;
 vi.mocked(invoke).mockResolvedValueOnce(snapshot).mockResolvedValueOnce({...compact,now_ms:5,rail_markers:[marker(1)]})
  .mockResolvedValue({...snapshot,paused:true,invalidated:true});
 const {container,rerender}=render(panel());
 await act(async()=>{});
 expect(Number(container.querySelector('circle')?.getAttribute('cx'))).toBeCloseTo(500,6);
 expect(invoke).toHaveBeenCalledTimes(1);
 await act(async()=>vi.advanceTimersByTime(499));
 expect(Number(container.querySelector('circle')?.getAttribute('cx'))).toBeCloseTo(500,6);
 await act(async()=>vi.advanceTimersByTime(1));
 expect(invoke).toHaveBeenCalledTimes(2);
 expect(invoke).toHaveBeenLastCalledWith('advance_operation',{knownConfigurationToken:'configuration-1'});
 expect(container.querySelector('circle')?.getAttribute('cx')).toBe(String(project(35,139.001,camera).x));
 rerender(panel({epoch:2,worldVersion:'two'}));
 expect(container.querySelector('circle')).toBeNull();
 await act(async()=>{});
 expect(container.querySelector('circle')).toBeNull();
});

test('marker stays on the painted diagonal at high zoom even when both ends are clipped',()=>{
 const zoomed={latitude:35.05,longitude:139.05,zoom:20};
 const diagonal={...marker(),alignment_start:{latitude:35,longitude:139},alignment_end:{latitude:35.1,longitude:139.1}};
 const {container,rerender}=render(<svg><OperationMapMarkers markers={[diagonal]} camera={zoomed} scale={1}/></svg>);
 const a=project(35,139,zoomed),b=project(35.1,139.1,zoomed);
 expect(a.x).toBeLessThan(0);
 expect(b.x).toBeGreaterThan(1000);
 const circle=container.querySelector('circle');
 expect(circle).not.toBeNull();
 expect(Number(circle?.getAttribute('cx'))).toBeCloseTo((a.x+b.x)/2,8);
 expect(Number(circle?.getAttribute('cy'))).toBeCloseTo((a.y+b.y)/2,8);
 expect(Math.abs(Number(circle?.getAttribute('cy'))-project(35.05,139.05,zoomed).y)).toBeGreaterThan(1);
 rerender(<svg><OperationMapMarkers markers={[{...diagonal,position:0}]} camera={zoomed} scale={1}/></svg>);
 expect(container.querySelector('circle')).toBeNull();
});


test('ordinary bus uses the same native snapshot and distinct passive road marker',async()=>{
 vi.useFakeTimers();
 const bus=(position:number)=>({key:{service_id:'bus:B1',scheduled_ms:0},edge_start:{latitude:35,longitude:138.999},edge_end:{latitude:35,longitude:139.001},position});
 const {services:_,...compact}=snapshot;
 vi.mocked(invoke).mockResolvedValueOnce({...snapshot,bus_markers:[bus(.25),bus(.5),{...bus(.6),key:{service_id:'12',scheduled_ms:0}}]})
  .mockResolvedValueOnce({...compact,now_ms:5,bus_markers:[bus(.75)]})
  .mockResolvedValueOnce({...compact,now_ms:9});
 const {container}=render(panel());
 await act(async()=>{});
 const group=()=>container.querySelector('[aria-label="表示できる走行中のバス"]');
 expect(group()?.getAttribute('pointer-events')).toBe('none');
 expect(group()?.querySelectorAll('circle')).toHaveLength(1);
 const x=()=>Number(group()?.querySelector('circle')?.getAttribute('cx'));
 const first=x();
 expect(first).toBeLessThan(500);
 expect(container.innerHTML).not.toContain('bus:B1');
 await act(async()=>vi.advanceTimersByTime(500));
 expect(x()).toBeGreaterThan(first);
 await act(async()=>vi.advanceTimersByTime(500));
 expect(group()).toBeNull();
 expect(container.querySelector('[aria-label="表示できる走行中の列車"]')).not.toBeNull();
 expect(invoke).toHaveBeenCalledTimes(3);
});

test('paused panning requests one read-only scoped snapshot after settling',async()=>{
 vi.useFakeTimers();
 const a={left:.1,right:.2,top:.3,bottom:.4},b={...a,left:.12},c={...a,left:.15};
 const snap=(bounds:typeof a)=><OperationProvider available blocked={false} epoch={1}
   worldVersion="one" vehicleViewport={bounds} onChange={()=>{}}>
   <svg><OperationVehicleLayer camera={camera} scale={2}/></svg>
 </OperationProvider>;
 vi.mocked(invoke).mockResolvedValue({...snapshot,paused:true});
 const {rerender}=render(snap(a));
 await act(async()=>{});
 expect(invoke).toHaveBeenCalledWith('operation_snapshot',{vehicleViewport:a});
 rerender(snap(b));
 await act(async()=>vi.advanceTimersByTime(80));
 rerender(snap(c));
 await act(async()=>vi.advanceTimersByTime(119));
 expect(invoke).toHaveBeenCalledTimes(1);
 await act(async()=>vi.advanceTimersByTime(1));
 expect(invoke).toHaveBeenCalledTimes(2);
 expect(invoke).toHaveBeenLastCalledWith('operation_snapshot',{vehicleViewport:c});
 await act(async()=>vi.advanceTimersByTime(1000));
 expect(invoke).toHaveBeenCalledTimes(2);
});

test('running panning supplies its latest scope to the existing clock tick',async()=>{
 vi.useFakeTimers();
 const a={left:.1,right:.2,top:.3,bottom:.4},b={...a,left:.12};
 const snap=(bounds:typeof a)=><OperationProvider available blocked={false} epoch={1}
    worldVersion="one" vehicleViewport={bounds} onChange={()=>{}}><div/></OperationProvider>;
 vi.mocked(invoke).mockResolvedValue(snapshot);
 const {rerender}=render(snap(a));
 await act(async()=>{});
 rerender(snap(b));
 expect(invoke).toHaveBeenCalledTimes(1);
 await act(async()=>vi.advanceTimersByTime(500));
 expect(invoke).toHaveBeenCalledTimes(2);
 expect(invoke).toHaveBeenLastCalledWith('advance_operation',{knownConfigurationToken:'configuration-1',vehicleViewport:b});
});

test('marker paint respects narrow-window SVG crop',()=>{
 const crop={x:400,y:0,width:200,height:600,scale:1};
 const pointOutside={...marker(),alignment_start:{latitude:35,longitude:138.9},alignment_end:{latitude:35,longitude:138.99}};
 const {container}=render(<svg><OperationMapMarkers markers={[pointOutside,marker()]} camera={camera} scale={1} crop={crop}/></svg>);
 expect(container.querySelectorAll('circle')).toHaveLength(1);
 expect(Number(container.querySelector('circle')?.getAttribute('cx'))).toBeCloseTo(500,6);
});
