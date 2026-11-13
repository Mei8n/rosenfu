// @vitest-environment jsdom
import React from "react";
import {cleanup,fireEvent,render,within} from "@testing-library/react";
import {afterEach,expect,test,vi} from "vitest";
import {App} from "./App";
import {DISPLAY_SETTINGS_KEY} from "./displaySettings";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn(),isTauri:()=>false}));
afterEach(()=>{cleanup();vi.restoreAllMocks();localStorage.removeItem(DISPLAY_SETTINGS_KEY);});
function save(background:string){localStorage.setItem(DISPLAY_SETTINGS_KEY,JSON.stringify({version:1,settings:{background}}));}
test("saved offline background creates no image even during initial render and persists online switches",()=>{
 save("sample");const href=vi.spyOn(Element.prototype,"setAttribute");const view=render(<App/>);
 expect(view.container.querySelectorAll("image")).toHaveLength(0);
 expect(href.mock.calls.some(([name,value])=>name==="href"&&/tile|gsi/.test(value))).toBe(false);
 const choices=within(view.container.querySelector('.background-choices') as HTMLElement);
 fireEvent.click(choices.getByRole("button",{name:"地理院地図（オンライン）"}));
 expect(view.container.querySelectorAll("image").length).toBeGreaterThan(0);
 expect(JSON.parse(localStorage.getItem(DISPLAY_SETTINGS_KEY)!).settings.background).toBe("gsi");
 view.unmount();const again=render(<App/>);expect(again.container.querySelector('image')?.getAttribute('href')).toContain("gsi");
 fireEvent.click(within(again.container.querySelector('.background-choices') as HTMLElement).getByRole("button",{name:"自作サンプル（オフライン）"}));
 again.unmount();const offline=render(<App/>);expect(offline.container.querySelectorAll('image')).toHaveLength(0);
});
test("fresh and old missing background retain OSM default",()=>{
 for(const raw of [null,JSON.stringify({version:0,theme:"dark"}),"broken"]){if(raw===null)localStorage.removeItem(DISPLAY_SETTINGS_KEY);else localStorage.setItem(DISPLAY_SETTINGS_KEY,raw);const view=render(<App/>);expect(view.container.querySelector('image')?.getAttribute('href')).toContain('tile.openstreetmap.org');view.unmount();}
});
test("denied saving keeps offline selection for session and exposes warning",()=>{
 const view=render(<App/>);vi.spyOn(Storage.prototype,'setItem').mockImplementation(()=>{throw Error('denied');});
 fireEvent.click(within(view.container.querySelector('.background-choices') as HTMLElement).getByRole('button',{name:'自作サンプル（オフライン）'}));
 expect(view.container.querySelectorAll('image')).toHaveLength(0);expect(view.container.querySelector('.display-settings-warning')?.textContent).toContain('保存できません');
});
