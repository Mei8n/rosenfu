import './testConstructionEnvironment';
// @vitest-environment jsdom
import React from "react";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { App } from "./App";
import { project, DEFAULT_CAMERA } from "./map";
import { alignmentLength, pointOnAlignment } from "./alignment";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
afterEach(() => { localStorage.removeItem("rosenfu.display-settings"); cleanup(); vi.resetAllMocks(); vi.unstubAllGlobals(); });
const alignment = { id:"3",owner:"2",name:"試験線",points:[{latitude:35.67504,longitude:139.72},{latitude:35.67504,longitude:139.78}] };
const empty = {next_id:"3",players:[{id:"1",name:"Player"}],companies:[{id:"2",owner:"1",name:"Railway"}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
const built = {...empty,next_id:"4",alignments:[alignment]};
const placed = {...built,next_id:"5",stations:[{id:"4",owner:"2",name:"新駅",latitude:35.67504,longitude:139.75}],station_placements:[{station_id:"4",alignment_id:"3",position:.5,length_m:200,width_m:12}]};
async function start(snapshot: unknown = empty) {
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke).mockImplementation(async command => {
    if(command==="engine_info") return {name:"Rosenfu",version:"construction-test"};
    if(command==="world_snapshot") return snapshot;
    throw Error("unexpected "+command);
  });
  render(<App/>); fireEvent.click(screen.getByRole("button",{name:"自作サンプル（オフライン）"})); await screen.findByText(/construction-test/);
  const map=screen.getByRole("img",{name:"駅配置用サンプル地図"});
  map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})});
  return map;
}
test("plan details appear only while hovering a plan, not elsewhere or during dragging",async()=>{
  vi.stubGlobal("PointerEvent", MouseEvent);
  const map=await start();
  fireEvent.click(map,{clientX:200,clientY:130});fireEvent.click(map,{clientX:300,clientY:170});
  expect(document.querySelector(".construction-tooltip")).toBeNull();
  fireEvent.keyDown(document.body,{key:"Escape"});
  const line=map.querySelector(".plan-hover-hit")!;
  fireEvent.pointerEnter(line,{clientX:250,clientY:150,pointerType:"mouse"});
  expect(document.querySelector(".construction-tooltip")?.textContent).toContain("線路計画");
  fireEvent.pointerLeave(line);
  expect(document.querySelector(".construction-tooltip")).toBeNull();
  fireEvent.pointerEnter(line,{clientX:250,clientY:150});
  fireEvent.pointerDown(map,{clientX:250,clientY:150,button:0,pointerId:1});
  expect(document.querySelector(".construction-tooltip")).toBeNull();
  fireEvent.pointerUp(map,{clientX:250,clientY:150,pointerId:1});
  fireEvent.pointerEnter(line,{clientX:250,clientY:150});
  fireEvent.pointerLeave(map);
  expect(document.querySelector(".construction-tooltip")).toBeNull();
});

test("drawing details follow the cursor without hover and diagnose an invalid candidate",async()=>{
 vi.stubGlobal("PointerEvent",MouseEvent);
 const map=await start();
 fireEvent.click(map,{clientX:200,clientY:130});fireEvent.click(map,{clientX:300,clientY:170});
 fireEvent.pointerMove(map,{clientX:360,clientY:190});
 expect(document.querySelector(".drawing-tooltip")?.textContent).toContain("速度上限");
 fireEvent.pointerMove(map,{clientX:220,clientY:160});
 expect(document.querySelector(".drawing-tooltip")?.textContent).toContain("線路が折り返しています");
 expect(map.querySelector(".invalid-candidate")).toBeTruthy();
 fireEvent.pointerMove(map,{clientX:297,clientY:168});
 expect(document.querySelector(".drawing-tooltip")?.textContent).not.toContain("最後の点をもう一度クリック");
 expect(map.querySelector(".invalid-candidate")).toBeNull();
 fireEvent.pointerLeave(map);expect(document.querySelector(".drawing-tooltip")).toBeNull();
});

test('hover only speed and highlight clear on leaving; small endpoints are visually distinct',async()=>{
 vi.stubGlobal('PointerEvent',MouseEvent);
 const map=await start();
 fireEvent.change(screen.getByRole('spinbutton',{name:'線路の目標最高速度'}),{target:{value:'80'}});
 fireEvent.click(map,{clientX:180,clientY:130});fireEvent.click(map,{clientX:250,clientY:130});fireEvent.click(map,{clientX:320,clientY:130});
 expect(document.querySelector('.curve-speed-label')).toBeNull();
 const anchors=map.querySelectorAll('.draft-anchor');
 expect(anchors[0].classList.contains('draft-endpoint')).toBe(true);
 expect(anchors[1].classList.contains('draft-endpoint')).toBe(false);
 expect(anchors[2].classList.contains('draft-endpoint')).toBe(true);
 expect(Number(anchors[1].getAttribute('width'))).toBeLessThan(8);
 const line=map.querySelector('.plan-hover-hit')!;
 fireEvent.pointerEnter(line,{clientX:220,clientY:130});
 expect(document.querySelector('.plan-hover-tooltip')?.textContent).toContain('80 km/h');
 expect(document.querySelector('.plan-hover-tooltip')?.textContent).toContain('高さ：未設定');
 expect(map.querySelectorAll('.plan-hovered')).toHaveLength(0);
 expect(map.querySelector('.plan-hover-point')).toBeTruthy();
 expect(document.querySelectorAll('.construction-tooltip')).toHaveLength(1);
 fireEvent.pointerLeave(line);
 expect(document.querySelector('.plan-hover-tooltip')).toBeNull();
 expect(map.querySelector('.plan-hovered')).toBeNull();
});

test('line and anchor focus markers share a compact radius without changing endpoint selection',async()=>{
 vi.stubGlobal('PointerEvent',MouseEvent);
 const map=await start();
 fireEvent.click(map,{clientX:180,clientY:130});fireEvent.click(map,{clientX:320,clientY:130});
 const line=map.querySelector('.plan-hover-hit')!;
 fireEvent.pointerEnter(line,{clientX:240,clientY:130});
 const point=map.querySelector('.plan-hover-point')!;
 const radius=point.getAttribute('r');
 expect(point.getAttribute('data-kind')).toBe('line');
 expect(point.parentElement?.getAttribute('pointer-events')).toBe('none');
 fireEvent.pointerLeave(line);
 fireEvent.click(map,{clientX:320,clientY:130});
 fireEvent.pointerMove(map,{clientX:320,clientY:130});
 const endpoint=map.querySelector('.endpoint-snap-ring')!;
 expect(endpoint.getAttribute('r')).toBe(radius);
 expect(endpoint.getAttribute('data-kind')).toBe('anchor');
 expect(endpoint.getAttribute('data-allowed')).toBe('true');
 expect(endpoint.parentElement?.querySelector('.focus-anchor-center')).not.toBeNull();
 // The visual change must not expand the actual endpoint snapping radius.
 fireEvent.pointerMove(map,{clientX:331,clientY:130});
 expect(map.querySelector('.endpoint-snap-ring')).toBeNull();
});

test('direct hover of the actual anchor hit rectangles shows start and middle anchor focus',async()=>{
 vi.stubGlobal('PointerEvent',MouseEvent);const map=await start();
 for(const x of [180,250,320])fireEvent.click(map,{clientX:x,clientY:130});
 fireEvent.keyDown(document.body,{key:'Escape'});
 for(const index of [0,1,2]){
  const anchor=map.querySelector(`[data-anchor-index="${index}"]`)!;
  fireEvent.pointerEnter(anchor,{clientX:180+70*index,clientY:130});
  expect(map.querySelector('.plan-hover-point')?.getAttribute('data-kind')).toBe('anchor');
  expect(map.querySelectorAll('.draft-anchor')).toHaveLength(3);
  fireEvent.pointerLeave(anchor);expect(map.querySelector('.plan-hover-point')).toBeNull();
 }
});
