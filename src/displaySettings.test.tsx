// @vitest-environment jsdom
import {act,cleanup,renderHook} from "@testing-library/react";
import {afterEach,expect,test,vi} from "vitest";
import {DEFAULT_DISPLAY_SETTINGS,DISPLAY_SETTINGS_KEY,parseDisplaySettings} from "./displaySettings";
import {useDisplaySettings} from "./useDisplaySettings";
afterEach(()=>{cleanup();vi.restoreAllMocks();localStorage.removeItem(DISPLAY_SETTINGS_KEY);});
test("old and missing fields merge defaults while invalid values and unknown versions reject",()=>{
 expect(parseDisplaySettings('{"theme":"dark"}')).toEqual({...DEFAULT_DISPLAY_SETTINGS,theme:"dark"});expect(parseDisplaySettings('{"version":0,"settings":{"lineWidth":6}}').lineWidth).toBe(6);
 expect(parseDisplaySettings('{"version":1,"settings":{"textSize":14}}')).toEqual({...DEFAULT_DISPLAY_SETTINGS,textSize:14});
 for(const raw of ['{"version":2,"settings":{}}','{"version":1,"settings":{"panelWidth":900}}','{"theme":"system"}','{"contrast":"100"}','{"textSize":12.5}','[]',"x".repeat(4097)])expect(()=>parseDisplaySettings(raw)).toThrow();
});
test("local preferences persist, restore and reset without touching other keys",()=>{
 localStorage.setItem("other-app","keep");const {result,unmount}=renderHook(useDisplaySettings);
 act(()=>{result.current.update({theme:"dark",lineWidth:7});result.current.update({contrast:120,panelWidth:360,textSize:15});});
 const expected={background:"osm",theme:"dark",lineWidth:7,contrast:120,panelWidth:360,textSize:15};expect(result.current.settings).toEqual(expected);unmount();const restored=renderHook(useDisplaySettings);expect(restored.result.current.settings).toEqual(expected);act(()=>restored.result.current.reset());expect(restored.result.current.settings).toEqual(DEFAULT_DISPLAY_SETTINGS);expect(localStorage.getItem(DISPLAY_SETTINGS_KEY)).toBeNull();expect(localStorage.getItem("other-app")).toBe("keep");localStorage.removeItem("other-app");
});
test("corrupt restore uses defaults and does not overwrite stored data automatically",()=>{
 localStorage.setItem(DISPLAY_SETTINGS_KEY,"bad JSON");const {result}=renderHook(useDisplaySettings);expect(result.current.settings).toEqual(DEFAULT_DISPLAY_SETTINGS);expect(result.current.error).toBe(true);expect(localStorage.getItem(DISPLAY_SETTINGS_KEY)).toBe("bad JSON");
});
test("denied storage keeps session changes and reset errors visible",()=>{
 vi.spyOn(Storage.prototype,"getItem").mockImplementation(()=>{throw Error("denied");});vi.spyOn(Storage.prototype,"setItem").mockImplementation(()=>{throw Error("full");});vi.spyOn(Storage.prototype,"removeItem").mockImplementation(()=>{throw Error("denied");});
 const {result}=renderHook(useDisplaySettings);expect(result.current.error).toBe(true);expect(result.current.settings).toEqual(DEFAULT_DISPLAY_SETTINGS);act(()=>result.current.update({theme:"dark"}));expect(result.current.settings.theme).toBe("dark");expect(result.current.status).toContain("このセッションのみ");act(()=>result.current.reset());expect(result.current.settings).toEqual(DEFAULT_DISPLAY_SETTINGS);expect(result.current.status).toContain("削除に失敗");
});

test("display settings size limit is UTF-8 bytes rather than UTF-16 character count",()=>{
 expect(parseDisplaySettings("{}".padEnd(4096," "))).toEqual(DEFAULT_DISPLAY_SETTINGS);
 const multibyte=JSON.stringify({theme:"light",note:"あ".repeat(1400)});expect(multibyte.length).toBeLessThan(4096);expect(()=>parseDisplaySettings(multibyte)).toThrow(/大きすぎ/);
});

test("background validates all providers and safely defaults old missing selection",()=>{
 for(const background of ["sample","gsi","osm"])expect(parseDisplaySettings(JSON.stringify({version:1,settings:{background}})).background).toBe(background);
 expect(parseDisplaySettings('{"version":1,"settings":{"theme":"dark"}}').background).toBe("osm");
 for(const background of [null,42,"other"])expect(()=>parseDisplaySettings(JSON.stringify({version:1,settings:{background}}))).toThrow();
});

test("new default lines are slimmer without overwriting explicitly saved widths",()=>{
 expect(DEFAULT_DISPLAY_SETTINGS.lineWidth).toBe(3);
 expect(parseDisplaySettings('{"version":1,"settings":{"lineWidth":4}}').lineWidth).toBe(4);
 expect(parseDisplaySettings('{"version":1,"settings":{"lineWidth":7}}').lineWidth).toBe(7);
 expect(parseDisplaySettings('{}').lineWidth).toBe(3);
});
