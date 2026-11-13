// @vitest-environment jsdom
import React from "react";
import {afterEach,expect,test,vi} from "vitest";
import {act,cleanup,fireEvent,render,screen,within} from "@testing-library/react";
import {App} from "./App";
vi.mock("@tauri-apps/api/core",()=>({isTauri:()=>false,invoke:vi.fn()}));
afterEach(()=>{cleanup();localStorage.clear();vi.useRealTimers();});
const finish=()=>act(()=>vi.advanceTimersByTime(200));
function setup(){vi.useFakeTimers();render(<App/>);const controls=document.querySelector('.camera-controls')!;return {controls,buttons:within(controls as HTMLElement),zoom:()=>Number(screen.getByText(/^ZL /).textContent!.match(/ZL ([\d.]+)/)![1])};}
test("buttons and keyboard use identical animated center zoom, accumulating repeats and reversing",()=>{
 const {buttons,zoom}=setup();
 fireEvent.click(buttons.getByRole('button',{name:'拡大'}));expect(zoom()).toBe(12);
 act(()=>vi.advanceTimersByTime(32));expect(zoom()).toBeGreaterThan(12);expect(zoom()).toBeLessThan(13);
 finish();expect(zoom()).toBe(13);
 fireEvent.keyDown(window,{key:'+',code:'Equal',shiftKey:true});act(()=>vi.advanceTimersByTime(32));expect(zoom()).toBeGreaterThan(13);expect(zoom()).toBeLessThan(14);
 fireEvent.keyDown(window,{key:'+',code:'Equal',shiftKey:true,repeat:true});finish();expect(zoom()).toBe(15);
 fireEvent.keyDown(window,{key:'-',code:'Minus'});act(()=>vi.advanceTimersByTime(32));expect(zoom()).toBeLessThan(15);
 fireEvent.keyDown(window,{key:'+',code:'NumpadAdd'});finish();expect(zoom()).toBe(15);
 fireEvent.click(buttons.getByRole('button',{name:'縮小'}));finish();expect(zoom()).toBe(14);
 fireEvent.keyDown(window,{key:'-',code:'NumpadSubtract'});finish();expect(zoom()).toBe(13);
});
test("zoom keys clamp boundaries and do not intercept typing, composition or browser modifiers",()=>{
 const {buttons,zoom}=setup();
 fireEvent.keyDown(window,{key:'-',code:'NumpadSubtract'});finish();expect(zoom()).toBe(12);
 const input=buttons.getByLabelText('中心緯度');
 for(const target of [input]){fireEvent.keyDown(target,{key:'+'});finish();expect(zoom()).toBe(12);}
 const editable=document.createElement('div');editable.contentEditable='true';Object.defineProperty(editable,'isContentEditable',{value:true});document.body.append(editable);
 fireEvent.keyDown(editable,{key:'+'});finish();expect(zoom()).toBe(12);editable.remove();
 for(const props of [{ctrlKey:true},{metaKey:true},{altKey:true},{isComposing:true}]){fireEvent.keyDown(window,{key:'+',...props});finish();expect(zoom()).toBe(12);}
 for(let n=0;n<8;n++)fireEvent.keyDown(window,{key:'+',code:'NumpadAdd',repeat:n>0});finish();expect(zoom()).toBe(16);
 expect(buttons.getByRole('button',{name:'拡大'}).hasAttribute('disabled')).toBe(true);
 for(let n=0;n<8;n++)fireEvent.keyDown(window,{key:'-',code:'NumpadSubtract',repeat:n>0});finish();expect(zoom()).toBe(12);
 expect(buttons.getByRole('button',{name:'縮小'}).hasAttribute('disabled')).toBe(true);
});
