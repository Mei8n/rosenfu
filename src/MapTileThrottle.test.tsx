// @vitest-environment jsdom
import {act,cleanup,fireEvent,render,screen} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {App} from './App';
import {visibleTiles} from './map';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:()=>false}));
vi.mock('./map',async importOriginal=>{
 const original=await importOriginal<typeof import('./map')>();
 return {...original,visibleTiles:vi.fn(original.visibleTiles)};
});
afterEach(()=>{cleanup();vi.useRealTimers();vi.unstubAllGlobals();vi.clearAllMocks();localStorage.clear();});
test('continuous pan requests fresh background tiles every 250 ms and catches the final position',()=>{
 vi.useFakeTimers();vi.stubGlobal('PointerEvent',MouseEvent);
 const view=render(<App/>);
 const map=screen.getByRole('img',{name:'駅配置用OpenStreetMap'});
 map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:1000,bottom:600,width:1000,height:600,toJSON:()=>({})});
 vi.mocked(visibleTiles).mockClear();
 fireEvent.pointerDown(map,{clientX:500,clientY:300,button:1});
 for(let i=1;i<=4;i++){
  fireEvent.pointerMove(map,{clientX:500+i*60,clientY:300,buttons:4});
  act(()=>vi.advanceTimersByTime(50));
 }
 expect(visibleTiles).not.toHaveBeenCalled();
 fireEvent.pointerMove(map,{clientX:800,clientY:300,buttons:4});
 act(()=>vi.advanceTimersByTime(50));
 expect(visibleTiles).toHaveBeenCalledTimes(1);
 const first=vi.mocked(visibleTiles).mock.calls[0][0];
 for(let i=1;i<=5;i++){
  fireEvent.pointerMove(map,{clientX:800+i*30,clientY:300,buttons:4});
  act(()=>vi.advanceTimersByTime(50));
 }
 expect(visibleTiles).toHaveBeenCalledTimes(2);
 expect(vi.mocked(visibleTiles).mock.calls[1][0]).not.toEqual(first);
 fireEvent.pointerMove(map,{clientX:980,clientY:300,buttons:4});
 fireEvent.pointerUp(map,{clientX:980,clientY:300,button:1});
 act(()=>vi.advanceTimersByTime(250));
 expect(visibleTiles).toHaveBeenCalledTimes(3);
 act(()=>vi.advanceTimersByTime(1000));
 expect(visibleTiles).toHaveBeenCalledTimes(3);
 fireEvent.pointerDown(map,{clientX:500,clientY:300,button:1});
 fireEvent.pointerMove(map,{clientX:700,clientY:300,buttons:4});
 view.unmount();act(()=>vi.advanceTimersByTime(1000));
 expect(visibleTiles).toHaveBeenCalledTimes(3);
});
