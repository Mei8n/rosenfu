// @vitest-environment jsdom
import {invoke} from '@tauri-apps/api/core';
import {act,cleanup,render,screen} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {RetainedRoadLayer,retainedRoadRect,useRetainedRoadView} from './RetainedRoadLayer';
import {cameraAt,project} from './map';

vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.useRealTimers();vi.resetAllMocks();});

const rect=(x:number,y:number)=>{
 const lon=(n:number)=>n/16384*360-180;
 const lat=(n:number)=>Math.atan(Math.sinh(Math.PI*(1-2*n/16384)))*180/Math.PI;
 return {west:lon(x+.1),east:lon(x+.9),north:lat(y+.1),south:lat(y+.9)};
};
const near=rect(14552,6451), far=rect(14624,6016);
const huge={west:130,south:30,east:144,north:44};
const response=(longitude:number,latitude:number)=>({
 requested_tiles:1,retained_tiles:1,segments:[[[longitude,latitude],[longitude+.0001,latitude]]],
 truncated:false,
});
function Harness({bounds=near,epoch=1,basis='one',enabled=true}:{
 bounds?:typeof near;epoch?:number;basis?:string;enabled?:boolean;
}){
 const road=useRetainedRoadView(enabled,bounds,epoch,basis);
 return <><output>{road.message}</output><svg><RetainedRoadLayer view={road.value} camera={cameraAt(35.675,139.75,15)}/></svg></>;
}
test('viewport converts actual map areas to z14 tile windows without allocating nationwide cells',()=>{
 expect(retainedRoadRect(near)).toEqual([14552,6451,14552,6451]);
 expect(retainedRoadRect(far)).toEqual([14624,6016,14624,6016]);
 expect(retainedRoadRect(huge)).toBeNull();
});
test('only the latest viewport begins after an earlier worker completes and its map is not reused across worlds',async()=>{
 vi.useFakeTimers();
 let finishA!:(value:ReturnType<typeof response>)=>void;
 let finishB!:(value:ReturnType<typeof response>)=>void;
 let finishC!:(value:ReturnType<typeof response>)=>void;
 vi.mocked(invoke).mockImplementationOnce(()=>new Promise(resolve=>{finishA=resolve;}));
 vi.mocked(invoke).mockImplementationOnce(()=>new Promise(resolve=>{finishB=resolve;}));
 vi.mocked(invoke).mockImplementationOnce(()=>new Promise(resolve=>{finishC=resolve;}));
 const app=render(<Harness/>);
 const layers=()=>app.container.querySelectorAll('[data-retained-road-reference]');
 await act(async()=>{await vi.advanceTimersByTimeAsync(180);});
 expect(invoke).toHaveBeenNthCalledWith(1,'retained_road_view',{rect:[14552,6451,14552,6451]});
 app.rerender(<Harness bounds={rect(14358,6506)}/>);
 await act(async()=>{await vi.advanceTimersByTimeAsync(180);});
 app.rerender(<Harness bounds={far}/>);
 await act(async()=>{await vi.advanceTimersByTimeAsync(180);});
 expect(invoke).toHaveBeenCalledTimes(1);
 await act(async()=>{finishA(response(139.75,35.67));});
 expect(layers()).toHaveLength(0);
 expect(invoke).toHaveBeenNthCalledWith(2,'retained_road_view',{rect:[14624,6016,14624,6016]});
 await act(async()=>{finishB(response(141.34,43.07));});
 expect(layers()).toHaveLength(1);
 expect(layers()[0].getAttribute('pointer-events')).toBe('none');

 app.rerender(<Harness bounds={far} epoch={2}/>);
 expect(layers()).toHaveLength(0);
 await act(async()=>{await vi.advanceTimersByTimeAsync(180);});
 expect(invoke).toHaveBeenCalledTimes(3);
 app.rerender(<Harness bounds={huge} epoch={2}/>);
 await act(async()=>{finishC(response(141.34,43.07));});
 expect(layers()).toHaveLength(0);
 expect(screen.getByText('道路を表示するには地図を拡大')).toBeTruthy();
 expect(invoke).toHaveBeenCalledTimes(3);
});
test('same-world evidence changes, including undo, remove the now-stale projection',async()=>{
 vi.useFakeTimers();
 vi.mocked(invoke).mockResolvedValue(response(139.75,35.67));
 const app=render(<Harness/>);
 await act(async()=>{await vi.advanceTimersByTimeAsync(180);});
 expect(app.container.querySelector('[data-retained-road-reference]')).not.toBeNull();
 app.rerender(<Harness basis='after undo'/>);
 expect(app.container.querySelector('[data-retained-road-reference]')).toBeNull();
 app.rerender(<Harness basis='after undo' enabled={false}/>);
 await act(async()=>{await vi.advanceTimersByTimeAsync(180);});
 expect(invoke).toHaveBeenCalledTimes(1);
});

test('camera frames pan and zoom the same projected geometry without rebuilding paths',()=>{
 const view={requested_tiles:1,retained_tiles:1,truncated:false,segments:[[[139.75,35.67],[139.751,35.67]]] as const};
 const app=render(<svg><RetainedRoadLayer view={view} camera={cameraAt(35.675,139.75,15)}/></svg>);
 const node=app.container.querySelector('[data-retained-road-reference]')!;
 const d=node.getAttribute('d'),transform=node.getAttribute('transform');
 const next=cameraAt(35.6755,139.7505,16);
 app.rerender(<svg><RetainedRoadLayer view={view} camera={next}/></svg>);
 expect(app.container.querySelector('[data-retained-road-reference]')).toBe(node);
 expect(node.getAttribute('d')).toBe(d);
 expect(node.getAttribute('transform')).not.toBe(transform);
 const [x,y]=d!.slice(1).split('L')[0]!.split(',').map(Number);
 const [dx,dy,scale]=node.getAttribute('transform')!.match(/[-.0-9]+/g)!.map(Number);
 const expected=project(35.67,139.75,next);
 expect(dx+scale*x).toBeCloseTo(expected.x,3);
 expect(dy+scale*y).toBeCloseTo(expected.y,3);
});

test('partial retained-road output is drawn and asks for a narrower view rather than reporting an error',async()=>{
 vi.useFakeTimers();
 vi.mocked(invoke).mockResolvedValue({...response(139.75,35.67),requested_tiles:2,retained_tiles:2,truncated:true});
 const app=render(<Harness bounds={{...near,east:139.77}}/>);
 await act(()=>vi.advanceTimersByTimeAsync(180));
 expect(app.container.querySelector('[data-retained-road-reference]')).not.toBeNull();
 expect(screen.getByText('見えている道路の一部を表示中。拡大すると絞れます')).toBeTruthy();
});
