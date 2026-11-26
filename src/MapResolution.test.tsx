// @vitest-environment jsdom
import React from "react";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { App } from "./App";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: () => false }));
afterEach(()=>{cleanup();localStorage.clear();vi.unstubAllGlobals();});
test("OSM uses the actual CSS viewport, not observed SVG drawing bounds, and offline requests no tiles",()=>{
 let resize: ResizeObserverCallback | undefined;
 const observe=vi.fn();
 vi.stubGlobal("ResizeObserver",class { constructor(callback:ResizeObserverCallback){resize=callback;} observe=observe; disconnect(){} });
 vi.stubGlobal("devicePixelRatio",1);
 render(<App/>);
 const map=screen.getByRole("img",{name:"駅配置用OpenStreetMap"});
 expect(observe).toHaveBeenCalledWith(map.closest(".map-surface"));
 let width=1000,height=600;
 map.getBoundingClientRect=()=>({x:0,y:0,right:width,bottom:height,left:0,top:0,width,height,toJSON:()=>({})});
 const zooms=()=>Array.from(map.querySelectorAll("image")).map(i=>i.getAttribute("data-tile-id")!.split("/")[0]);
 expect(zooms().length).toBeGreaterThan(0);
 expect(zooms().every(z=>z==="12")).toBe(true);
 const first=[...map.querySelectorAll("image")].map(image=>image.getAttribute("data-tile-id"));
 const resized=(w:number,h:number)=>act(()=>resize!([{contentRect:{width:w,height:h}} as ResizeObserverEntry],{} as ResizeObserver));
 resized(64592,71388);
 expect([...map.querySelectorAll("image")].map(image=>image.getAttribute("data-tile-id"))).toEqual(first);
 width=1920;height=1080;
 // A true layout change must still be detected if the browser reports stale
 // or drawing-space dimensions in its observer entry.
 resized(1000,600);
 expect(zooms().length).toBeGreaterThan(0);
 expect(zooms().every(z=>z==="13")).toBe(true);
 vi.stubGlobal("devicePixelRatio",2);fireEvent(window,new Event("resize"));
 // A bounded request budget may select a lower detail level on very dense displays.
 expect(zooms().every(z=>Number(z)>=13)).toBe(true);
 fireEvent.click(screen.getByRole("button",{name:"自作サンプル（オフライン）"}));
 expect(map.querySelectorAll("image")).toHaveLength(0);
 fireEvent(window,new Event("resize"));
 expect(map.querySelectorAll("image")).toHaveLength(0);
});
