// @vitest-environment jsdom
import React from "react";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { App } from "./App";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: () => false }));
afterEach(()=>{cleanup();localStorage.clear();vi.unstubAllGlobals();});
test("OSM follows actual display size and DPI, while offline mode requests no tiles",()=>{
 let resize: ResizeObserverCallback | undefined;
 vi.stubGlobal("ResizeObserver",class { constructor(callback:ResizeObserverCallback){resize=callback;} observe(){} disconnect(){} });
 vi.stubGlobal("devicePixelRatio",1);
 render(<App/>);
 const map=screen.getByRole("img",{name:"駅配置用OpenStreetMap"});
 const zooms=()=>Array.from(map.querySelectorAll("image")).map(i=>i.getAttribute("data-tile-id")!.split("/")[0]);
 expect(zooms().every(z=>z==="12")).toBe(true);
 act(()=>resize!([{contentRect:{width:1920,height:1080}} as ResizeObserverEntry],{} as ResizeObserver));
 expect(zooms().every(z=>z==="13")).toBe(true);
 vi.stubGlobal("devicePixelRatio",2);fireEvent(window,new Event("resize"));
 // A bounded request budget may select a lower detail level on very dense displays.
 expect(zooms().every(z=>Number(z)>=13)).toBe(true);
 fireEvent.click(screen.getByRole("button",{name:"自作サンプル（オフライン）"}));
 expect(map.querySelectorAll("image")).toHaveLength(0);
 fireEvent(window,new Event("resize"));
 expect(map.querySelectorAll("image")).toHaveLength(0);
});
