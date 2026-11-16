import {settledMapClick} from './testMapClick';
// @vitest-environment jsdom
import React from 'react';
import {afterEach,expect,test,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {App} from './App';
import {project,mapViewport} from './map';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:vi.fn()}));
// Geometry/type tests deliberately have no road reference. Road acquisition and
// its deadline are covered by ConstructionRoadAcceptance, not real network calls.
vi.mock('./roadProvider',()=>({createRoadProvider:()=>({name:'unavailable fixture',load:async()=>{throw Error('road reference unavailable in geometry fixture');}})}));
afterEach(()=>{cleanup();localStorage.clear();vi.resetAllMocks();vi.unstubAllGlobals();});
test('circular hover guide precedes saved geometry and undo/redo restores the path',async()=>{
 vi.stubGlobal("PointerEvent",MouseEvent);
 const world={next_id:'3',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'R'}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async command=>command==='preview_construction_plan'?{validationError:null}:command==='engine_info'?{name:'Rosenfu',version:'circle-test'}:world);
 render(<App/>);fireEvent.click(screen.getByRole('button',{name:'自作サンプル（オフライン）'}));await screen.findByText(/circle-test/);
 const map=screen.getByRole('img',{name:'駅配置用サンプル地図'});
 map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 await settledMapClick(map,{clientX:180,clientY:130});await settledMapClick(map,{clientX:250,clientY:130});fireEvent.pointerMove(map,{clientX:300,clientY:180});
 expect(map.querySelector('.curve-circle-guide')).not.toBeNull();
 await settledMapClick(map,{clientX:300,clientY:180});
 const preview=map.querySelector('.draft-path')!.getAttribute('d');
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});expect(map.querySelector('.curve-circle-guide')).toBeNull();
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});expect(map.querySelector('.draft-path')!.getAttribute('d')).toBe(preview);
 fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('build_alignment',expect.any(Object)));
 const call=vi.mocked(invoke).mock.calls.find(([command])=>command==='build_alignment')!;
 expect(call).toBeTruthy();
 const points=(call[1] as {points:{latitude:number;longitude:number}[]}).points;
 expect(points.map((point,index)=>{const p=project(point.latitude,point.longitude);return(index?'L':'M')+p.x+','+p.y;}).join(' ')).toBe(preview);
});

test('automatic straights and curves need no mode switch, use identical preview/commit and cap speeds',async()=>{
 vi.stubGlobal('PointerEvent',MouseEvent);
 const world={next_id:'3',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'R'}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async command=>command==='preview_construction_plan'?{validationError:null}:command==='engine_info'?{name:'Rosenfu',version:'auto-test'}:world);
 render(<App/>);fireEvent.click(screen.getByRole('button',{name:'自作サンプル（オフライン）'}));await screen.findByText(/auto-test/);
 expect(screen.queryByRole('button',{name:/^(直線|曲線)$/})).toBeNull();
 const speed=screen.getByRole('spinbutton',{name:'線路の目標最高速度'});
 fireEvent.change(speed,{target:{value:'80'}});
 const map=screen.getByRole('img',{name:'駅配置用サンプル地図'});
 map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 await settledMapClick(map,{clientX:180,clientY:130});await settledMapClick(map,{clientX:250,clientY:130});
 fireEvent.pointerMove(map,{clientX:330,clientY:134});
 expect(map.querySelector('.curve-circle-guide')).toBeNull();
 expect(document.querySelector('.drawing-tooltip')?.textContent).toContain('80 km/h');
 await settledMapClick(map,{clientX:330,clientY:134});
 const anchor=map.querySelectorAll('.draft-anchor')[2];expect(Number(anchor.getAttribute('y'))+Number(anchor.getAttribute('height'))/2).toBeCloseTo(260,5);
 const straightPath=map.querySelector('.draft-path')!.getAttribute('d')!;
 fireEvent.pointerMove(map,{clientX:380,clientY:180});expect(map.querySelector('.curve-circle-guide')).toBeTruthy();
 expect(document.querySelector('.curve-speed-label')).toBeNull();
 await settledMapClick(map,{clientX:380,clientY:180});
 expect(map.querySelector('.draft-path')!.getAttribute('d')).toContain(straightPath);
 const finalPreview=map.querySelector('.draft-path')!.getAttribute('d');
 fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('build_alignment',expect.any(Object)));
 const call=vi.mocked(invoke).mock.calls.find(([command])=>command==='build_alignment')!;
 expect(call[1]).toMatchObject({designSpeedKmh:80});
 const points=(call[1] as {points:{latitude:number;longitude:number}[]}).points;
 expect(points.map((point,index)=>{const p=project(point.latitude,point.longitude);return(index?'L':'M')+p.x+','+p.y;}).join(' ')).toBe(finalPreview);
});

test('outgoing tangent guide follows the preview endpoint and extends beyond the viewport',async()=>{
 vi.stubGlobal('PointerEvent',MouseEvent);
 const world={next_id:'3',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'R'}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async command=>command==='preview_construction_plan'?{validationError:null}:command==='engine_info'?{name:'Rosenfu',version:'tangent-test'}:world);
 render(<App/>);fireEvent.click(screen.getByRole('button',{name:'自作サンプル（オフライン）'}));await screen.findByText(/tangent-test/);
 const map=screen.getByRole('img',{name:'駅配置用サンプル地図'});
 map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 const guide=()=>map.querySelector('.construction-tangent-guide');
 await settledMapClick(map,{clientX:180,clientY:130});expect(guide()).toBeNull();
 await settledMapClick(map,{clientX:250,clientY:130});
 expect(Number(guide()!.getAttribute('y1'))).toBeCloseTo(Number(guide()!.getAttribute('y2')),5);
 expect(guide()!.getAttribute('pointer-events')).toBe('none');
 fireEvent.pointerMove(map,{clientX:300,clientY:180});
 expect(map.querySelector('.curve-circle-guide')).toBeTruthy();
 expect(Number(guide()!.getAttribute('x1'))).toBeCloseTo(600,5);
 expect(Number(guide()!.getAttribute('y1'))).toBeCloseTo(360,5);
 expect(Number(guide()!.getAttribute('x2'))).toBeCloseTo(600,4);
 expect(Number(guide()!.getAttribute('y2'))).toBeGreaterThan(600);
 const curvedGuide=guide()!.outerHTML;
 fireEvent.pointerMove(map,{clientX:320,clientY:160});
 expect(guide()!.outerHTML).not.toBe(curvedGuide);
 const dx=Number(guide()!.getAttribute('x2'))-Number(guide()!.getAttribute('x1'));
 const dy=Number(guide()!.getAttribute('y2'))-Number(guide()!.getAttribute('y1'));
 expect(Math.hypot(dx,dy)).toBeGreaterThan(600);
 expect(Number(guide()!.getAttribute('x2'))>1000 || Number(guide()!.getAttribute('y2'))>600).toBe(true);
 // No invented forward direction for a refused curve.
 fireEvent.pointerMove(map,{clientX:220,clientY:190});
 expect(map.querySelector('.invalid-candidate')).toBeTruthy();
 expect(guide()).toBeNull();
 await settledMapClick(map,{clientX:300,clientY:180});
 expect(Number(guide()!.getAttribute('x1'))).toBeCloseTo(600,5);
 expect(Number(guide()!.getAttribute('y1'))).toBeCloseTo(360,5);
 expect(Number(guide()!.getAttribute('x2'))).toBeCloseTo(600,4);
 expect(Number(guide()!.getAttribute('y2'))).toBeGreaterThan(360);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 expect(Number(guide()!.getAttribute('x1'))).toBeCloseTo(500,5);
 expect(Number(guide()!.getAttribute('y1'))).toBeCloseTo(Number(guide()!.getAttribute('y2')),5);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});
 await settledMapClick(map,{clientX:300,clientY:180});
 expect(guide()).toBeNull();
 fireEvent.click(screen.getByRole('tab',{name:'駅設置'}));
 expect(guide()).toBeNull();
});

test('Ctrl bypasses straight snapping until release, including stationary preview and committed points',async()=>{
 vi.stubGlobal('PointerEvent',MouseEvent);
 const world={next_id:'3',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'R'}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async command=>command==='preview_construction_plan'?{validationError:null}:command==='engine_info'?{name:'Rosenfu',version:'ctrl-test'}:world);
 render(<App/>);fireEvent.click(screen.getByRole('button',{name:'自作サンプル（オフライン）'}));await screen.findByText(/ctrl-test/);
 const map=screen.getByRole('img',{name:'駅配置用サンプル地図'});
 map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
 await settledMapClick(map,{clientX:180,clientY:130});await settledMapClick(map,{clientX:250,clientY:130});
 fireEvent.pointerMove(map,{clientX:330,clientY:134});
 expect(map.querySelector('.curve-circle-guide')).toBeNull();
 fireEvent.keyDown(document.body,{key:'Control',ctrlKey:true});
 expect(map.querySelector('.curve-circle-guide')).not.toBeNull();
 fireEvent.keyUp(document.body,{key:'Control',ctrlKey:false});
 expect(map.querySelector('.curve-circle-guide')).toBeNull();
 fireEvent.pointerMove(map,{clientX:330,clientY:134,ctrlKey:true});
 expect(map.querySelector('.curve-circle-guide')).not.toBeNull();
 await settledMapClick(map,{clientX:330,clientY:134,ctrlKey:true});
 const anchor=map.querySelectorAll('.draft-anchor')[2];
 expect(Number(anchor.getAttribute('y'))+Number(anchor.getAttribute('height'))/2).toBeCloseTo(268,5);
 const committed=map.querySelector('.draft-path')!.getAttribute('d');
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});
 expect(map.querySelector('.draft-path')!.getAttribute('d')).toBe(committed);
 fireEvent.blur(window);
 expect(map.querySelector('.curve-circle-guide')).toBeNull();
});

test('tangent guide reaches past resized viewport edges at different zoom levels',async()=>{
 vi.stubGlobal('PointerEvent',MouseEvent);
 let resize: (entries: {contentRect:{width:number;height:number}}[])=>void=()=>{};
 vi.stubGlobal('ResizeObserver',class {constructor(callback:typeof resize){resize=callback;}observe(){}disconnect(){}});
 const world={next_id:'3',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'R'}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
 vi.mocked(isTauri).mockReturnValue(true);
 vi.mocked(invoke).mockImplementation(async command=>command==='preview_construction_plan'?{validationError:null}:command==='engine_info'?{name:'Rosenfu',version:'guide-resize-test'}:world);
 render(<App/>);fireEvent.click(screen.getByRole('button',{name:'自作サンプル（オフライン）'}));await screen.findByText(/guide-resize-test/);
 const map=screen.getByRole('img',{name:'駅配置用サンプル地図'});
 let width=1000,height=600;
 map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:width,bottom:height,width,height,toJSON:()=>({})});
 await settledMapClick(map,{clientX:360,clientY:260});await settledMapClick(map,{clientX:500,clientY:260});
 fireEvent.pointerMove(map,{clientX:600,clientY:360});
 for(const size of [{width:1920,height:400},{width:400,height:1200},{width:2560,height:1440}]){
   ({width,height}=size);
   await act(async()=>resize([{contentRect:size}]));
   // No pointer movement: the observed dimensions must supersede old cursor data.
   const check=()=>{
     const line=map.querySelector('.construction-tangent-guide')!;
     expect(line).toBeTruthy();
     const end={x:Number(line.getAttribute('x2')),y:Number(line.getAttribute('y2'))};
     const viewport=mapViewport(width,height);
     expect(Number.isFinite(end.x)&&Number.isFinite(end.y)).toBe(true);
     expect(end.x<viewport.x||end.x>viewport.x+viewport.width||end.y<viewport.y||end.y>viewport.y+viewport.height).toBe(true);
   };
   check();
   fireEvent.click(screen.getByRole('button',{name:'拡大'}));check();
   fireEvent.click(screen.getByRole('button',{name:'縮小'}));check();
 }
});
