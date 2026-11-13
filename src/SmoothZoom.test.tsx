// @vitest-environment jsdom
import React from "react";
import {afterEach,expect,test,vi} from "vitest";
import {act,cleanup,fireEvent,render,renderHook} from "@testing-library/react";
import {useSmoothZoom,wheelZoomDelta} from "./useSmoothZoom";
import {MAX_ZOOM,cameraAt,project,unproject,visibleTiles,positionTile} from "./map";
import type {Camera} from "./map";
import {GsiBackground} from "./GsiBackground";
afterEach(()=>{cleanup();vi.useRealTimers();vi.unstubAllGlobals();});
test("small wheel deltas accumulate into fractional cursor-anchored frames",()=>{
 vi.useFakeTimers();const camera={current:cameraAt(35.675,139.75,13)},frames:Camera[]=[];
 const anchor={x:300,y:250},point=unproject(anchor.x,anchor.y,camera.current);
 const hook=renderHook(()=>useSmoothZoom(camera,next=>{camera.current=next;frames.push(next);}));
 act(()=>hook.result.current.zoomBy(.25,anchor));
 act(()=>vi.advanceTimersByTime(32));
 expect(camera.current.zoom).toBeGreaterThan(13);expect(camera.current.zoom).toBeLessThan(13.25);
 act(()=>hook.result.current.zoomBy(.25,anchor));act(()=>vi.advanceTimersByTime(200));
 expect(camera.current.zoom).toBeCloseTo(13.5,9);
 for(const frame of frames){const p=project(point.latitude,point.longitude,frame);expect(p.x).toBeCloseTo(anchor.x,7);expect(p.y).toBeCloseTo(anchor.y,7);}
});
test("direction reversal, cancellation, bounds and unmount discard obsolete animation",()=>{
 vi.useFakeTimers();const camera={current:cameraAt(35.675,139.75,MAX_ZOOM-.1)},write=vi.fn((next:Camera)=>{camera.current=next;});
 const hook=renderHook(()=>useSmoothZoom(camera,write));const anchor={x:500,y:300};
 act(()=>hook.result.current.zoomBy(10,anchor));act(()=>vi.advanceTimersByTime(200));expect(camera.current.zoom).toBe(MAX_ZOOM);
 act(()=>hook.result.current.zoomBy(-.5,anchor));act(()=>vi.advanceTimersByTime(32));
 act(()=>hook.result.current.zoomBy(.5,anchor));act(()=>vi.advanceTimersByTime(200));expect(camera.current.zoom).toBe(MAX_ZOOM);
 act(()=>hook.result.current.zoomBy(-1,anchor));act(()=>vi.advanceTimersByTime(32));act(()=>hook.result.current.cancel());const stopped=camera.current;
 act(()=>vi.advanceTimersByTime(200));expect(camera.current).toBe(stopped);
 act(()=>hook.result.current.zoomBy(-1,anchor));hook.unmount();const count=write.mock.calls.length;act(()=>vi.advanceTimersByTime(200));expect(write).toHaveBeenCalledTimes(count);
});
test("wheel units normalize and reduced motion avoids animation",()=>{
 expect(wheelZoomDelta(-100,0,600)).toBe(.25);expect(wheelZoomDelta(-3,1,600)).toBe(.12);expect(wheelZoomDelta(-1,2,600)).toBe(.75);
 expect(wheelZoomDelta(NaN,0,600)).toBe(0);
 vi.stubGlobal("matchMedia",()=>({matches:true}));const camera={current:cameraAt(35,139,13)};
 const hook=renderHook(()=>useSmoothZoom(camera,next=>{camera.current=next;}));act(()=>hook.result.current.zoomBy(.25,{x:500,y:300}));expect(camera.current.zoom).toBe(13.25);
});
test("loaded tiles remain georeferenced while replacements load, and failures stay explicit",()=>{
 const oldCamera=cameraAt(35.675,139.75,12),nextCamera=cameraAt(35.675,139.75,13.25);
 const oldTiles=visibleTiles(oldCamera,"osm"),nextTiles=visibleTiles(nextCamera,"osm");const status=vi.fn();
 const view=render(<svg><GsiBackground tiles={oldTiles} camera={oldCamera} width={1000} height={600} onStatus={status}/></svg>);
 for(const image of view.container.querySelectorAll('image'))fireEvent.load(image);
 const oldElement=view.container.querySelector('image')!,oldId=oldElement.getAttribute('data-tile-id')!;
 view.rerender(<svg><GsiBackground tiles={nextTiles} camera={nextCamera} width={1000} height={600} onStatus={status}/></svg>);
 const retained=view.container.querySelector(`image[data-tile-id="${oldId}"]`)!;expect(retained).toBe(oldElement);expect(retained.hasAttribute('data-fallback-tile')).toBe(true);
 expect(Number(retained.getAttribute('width'))).toBeCloseTo(positionTile(oldTiles[0],nextCamera).width,7);
 for(const image of view.container.querySelectorAll('image:not([data-fallback-tile])'))fireEvent.error(image);
 expect(view.container.querySelectorAll('[data-fallback-tile]')).toHaveLength(0);
 expect(view.container.querySelectorAll('[data-failed-tile]')).toHaveLength(nextTiles.length);
 expect(status.mock.calls.at(-1)?.[0]).toEqual({loaded:0,failed:nextTiles.length,total:nextTiles.length});
 view.unmount();
});
test("animation measures elapsed time in the rAF clock even with a different performance origin",()=>{
 const callbacks:Array<FrameRequestCallback>=[];
 vi.stubGlobal('requestAnimationFrame',(callback:FrameRequestCallback)=>{callbacks.push(callback);return callbacks.length;});
 vi.stubGlobal('cancelAnimationFrame',vi.fn());
 const camera={current:cameraAt(35,139,13)};
 const hook=renderHook(()=>useSmoothZoom(camera,next=>{camera.current=next;}));
 act(()=>hook.result.current.zoomBy(1,{x:500,y:300}));
 act(()=>callbacks.shift()!(100000));expect(camera.current.zoom).toBe(13);
 act(()=>callbacks.shift()!(100080));expect(camera.current.zoom).toBeGreaterThan(13);expect(camera.current.zoom).toBeLessThan(14);
 act(()=>callbacks.shift()!(100160));expect(camera.current.zoom).toBe(14);
});
