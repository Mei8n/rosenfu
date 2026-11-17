import {settledPointerMove} from './testMapClick';
// @vitest-environment jsdom
import React from "react";
import {settledMapClick} from "./testMapClick";
import {afterEach,expect,test,vi} from "vitest";
import {act,cleanup,fireEvent,render,screen} from "@testing-library/react";
import {invoke,isTauri} from "@tauri-apps/api/core";
import {App} from "./App";
import {coordinatesFromClient,DEFAULT_CAMERA} from "./map";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn(),isTauri:vi.fn()}));
window.PointerEvent=MouseEvent as typeof PointerEvent;
afterEach(()=>{cleanup();vi.resetAllMocks();localStorage.clear();});
const rect={x:0,y:0,left:0,top:0,right:500,bottom:300,width:500,height:300,toJSON:()=>({})};
const point=(x:number,y:number)=>coordinatesFromClient(x,y,rect,DEFAULT_CAMERA)!;
const line={id:"3",owner:"2",name:"既存",track_type:"normal",points:[point(180,130),point(250,130)]};
const world={next_id:"4",players:[{id:"1",name:"P"}],companies:[{id:"2",owner:"1",name:"C"}],stations:[],tracks:[],routes:[],services:[],alignments:[line],station_placements:[]};
async function start(extra={}){
 vi.mocked(isTauri).mockReturnValue(true);vi.mocked(invoke).mockImplementation(async command=>{
 if(command==="preview_construction_plan")return {validationError:null,newCrossingCount:0,newCrossings:[],roadsVerified:false};if(command==="engine_info")return {name:"Rosenfu",version:"extension-test"};if(command==="world_snapshot")return {...world,...extra};throw Error("unexpected "+command);
 });render(<App/>);fireEvent.click(screen.getByRole("button",{name:"自作サンプル（オフライン）"}));await screen.findByText(/extension-test/);
 // Startup also restores plans and camera; finish those effects before injecting pointer input.
 await act(async()=>{});
 const map=screen.getByRole("img",{name:"駅配置用サンプル地図"});map.getBoundingClientRect=()=>rect;return map;
}
for(const [sourceCount,targetCount] of [[2,1],[2,4]])for(const fresh of [false,true])for(const sourceEnd of ['start','end'] as const)for(const targetEnd of ['start','end'] as const)test(`mixed-track ${sourceCount} to ${targetCount}, ${fresh?'new':'built'} ${sourceEnd} to ${targetEnd} preserves Undo/Redo connection`,async()=>{
 const x=sourceEnd==='end'?350:100,far=sourceEnd==='end'?420:40,startX=sourceEnd==='end'?250:180;
 const source={...line,infrastructure:[{level:'ground',track_count:sourceCount}]};
 const target={...line,id:'4',points:targetEnd==='start'?[point(x,130),point(far,130)]:[point(far,130),point(x,130)],infrastructure:[{level:'ground',track_count:targetCount}]};
 const map=await start({alignments:fresh?[target]:[source,target]});
 fireEvent.change(screen.getByLabelText('敷設する線路数'),{target:{value:String(sourceCount)}});
 await settledMapClick(map,{clientX:startX,clientY:130});
 await settledPointerMove(map,{clientX:x+2,clientY:131});
 expect(map.querySelector('.destination-snap-ring')?.getAttribute('data-allowed')).toBe('true');
 await settledMapClick(map,{clientX:x+2,clientY:131});
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 const previous=vi.mocked(invoke).getMockImplementation()!;vi.mocked(invoke).mockImplementation((command,args)=>command==='build_plan'?Promise.reject(Error('inspect mixed-track request')):previous(command,args));
 fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));await screen.findByText(/inspect mixed-track request/);
 const request=vi.mocked(invoke).mock.calls.find(([c])=>c==='build_plan')![1] as any;
 expect(request.alignments[0].destination).toEqual({alignmentId:'4',endpoint:targetEnd});
 expect(request.alignments[0].infrastructure.track_count).toBe(sourceCount);
 expect(request.alignments[0].points.at(-1)).toEqual(point(x,130));
});
