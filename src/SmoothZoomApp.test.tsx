// @vitest-environment jsdom
import React from "react";
import {afterEach,expect,test,vi} from "vitest";
import {act,cleanup,fireEvent,render,screen} from "@testing-library/react";
import {App} from "./App";
import {invoke} from "@tauri-apps/api/core";
vi.mock("@tauri-apps/api/core",()=>({isTauri:()=>false,invoke:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.useRealTimers();});
test("map wheel animates fractional zoom without changing world data and cancels at explicit navigation",()=>{
 vi.useFakeTimers();render(<App/>);
 const map=screen.getByRole("img",{name:"駅配置用OpenStreetMap"});
 map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:1000,bottom:600,width:1000,height:600,toJSON:()=>({})});
 const wheel=(deltaY:number)=>fireEvent(map,new WheelEvent('wheel',{bubbles:true,cancelable:true,deltaY,clientX:300,clientY:250}));
 const initial=Array.from(map.querySelectorAll('image')).map(i=>i.getAttribute('href'));
 wheel(-100);act(()=>vi.advanceTimersByTime(32));
 expect(screen.queryByText('ZL 12 / Web Mercator')).toBeNull();
 expect(Array.from(map.querySelectorAll('image')).map(i=>i.getAttribute('href'))).toEqual(initial);
 act(()=>vi.advanceTimersByTime(160));expect(screen.getByText('ZL 12.25 / Web Mercator')).toBeTruthy();
 wheel(-100);act(()=>vi.advanceTimersByTime(32));
 fireEvent.click(screen.getByRole('button',{name:'東京へ戻る'}));act(()=>vi.advanceTimersByTime(300));
 expect(screen.getByText('ZL 12 / Web Mercator')).toBeTruthy();
 wheel(-100);act(()=>vi.advanceTimersByTime(32));
 fireEvent.click(screen.getByRole('button',{name:'自作サンプル（オフライン）'}));
 const label=document.querySelector('.camera-controls')!.textContent;
 act(()=>vi.advanceTimersByTime(300));expect(document.querySelector('.camera-controls')!.textContent).toBe(label);
 expect(map.querySelectorAll('image')).toHaveLength(0);expect(invoke).not.toHaveBeenCalled();
});
