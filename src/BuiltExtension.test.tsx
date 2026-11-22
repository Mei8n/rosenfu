import {replyToCommandOnce} from './testInvoke';
import {settledMapClick} from './testMapClick';
// @vitest-environment jsdom
import React from "react";
import styles from "./style.css?raw";
import {afterEach,expect,test,vi} from "vitest";
import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
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
 vi.mocked(isTauri).mockReturnValue(true);vi.mocked(invoke).mockImplementation(async command=>{if(command==="preview_construction_plan")return {validationError:null};
 if(command==="engine_info")return {name:"Rosenfu",version:"extension-test"};if(command==="world_snapshot")return {...world,...extra};throw Error("unexpected "+command);
 });render(<App/>);fireEvent.click(screen.getByRole("button",{name:"自作サンプル（オフライン）"}));await screen.findByText(/extension-test/);
 // Startup also restores plans and camera; finish those effects before injecting pointer input.
 await act(async()=>{});
 const map=screen.getByRole("img",{name:"駅配置用サンプル地図"});map.getBoundingClientRect=()=>rect;return map;
}
for(const endpoint of ["start","end"] as const)test(`${endpoint} extension joins existing ID inside a multi-path plan and preserves station dimensions`,async()=>{
 const map=await start();const click=async (x:number,y=130)=>await settledMapClick(map,{clientX:x,clientY:y,detail:1});
 const anchor=endpoint==="start"?180:250,finish=endpoint==="start"?120:310;
 await click(anchor+2);expect(screen.getByText(/から延伸を計画しています/)).toBeTruthy();await click(finish);
 fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));fireEvent.change(screen.getByLabelText("駅の長さ"),{target:{value:"8"}});await click((anchor+finish)/2);
 fireEvent.click(screen.getByRole("tab",{name:"線路"}));await click(finish);await click(340,220);await click(400,220);
 const original=vi.mocked(invoke).getMockImplementation()!;vi.mocked(invoke).mockImplementation((command,args)=>command==="build_plan"?Promise.reject(Error("atomic failure")):original(command,args));fireEvent.click(screen.getByRole("button",{name:"計画を着工"}));await screen.findByText(/atomic failure/);
 const request=vi.mocked(invoke).mock.calls.find(([c])=>c==="build_plan")![1] as {alignments:any[]};
 expect(request.alignments).toHaveLength(2);const ext=request.alignments.find(p=>p.extension);
 expect(ext.extension).toEqual({alignmentId:"3",endpoint});expect(ext.points[0]).toEqual(line.points[endpoint==="start"?0:1]);expect(ext.stations[0].lengthM).toBe(160);
 expect(map.querySelectorAll(".planned-station")).toHaveLength(1);
 replyToCommandOnce('build_plan',world);fireEvent.click(screen.getByRole("button",{name:"計画を着工"}));await waitFor(()=>expect(map.querySelectorAll(".draft-anchor")).toHaveLength(0));
});
test("attributed endpoints can start an extension without duplicating the alignment",async()=>{
 const map=await start({alignment_provenance:[{alignment_id:"3",source:{source_key:"x"}}]});
 await settledMapClick(map,{clientX:250,clientY:130});expect(await screen.findByText(/から延伸を計画しています/)).toBeTruthy();expect(map.querySelectorAll(".draft-anchor")).toHaveLength(1);
});

test('station construction is final and extension retains its built station without replay',async()=>{
 const map=await start();
 const station={id:"4",owner:"2",name:"駅",...point(215,130)};
 const placement={station_id:"4",alignment_id:"3",position:0.5,length_m:200,width_m:12};
 fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));
 replyToCommandOnce('build_plan',{...world,stations:[station],station_placements:[placement]});
 await settledMapClick(map,{clientX:215,clientY:130});fireEvent.click(screen.getByRole("button",{name:"計画を着工"}));await waitFor(()=>expect(invoke).toHaveBeenCalledWith("build_plan",expect.anything()));
 await screen.findByText(/計画駅を建設しました/);
 fireEvent.keyDown(document.body,{key:"z",ctrlKey:true});expect(vi.mocked(invoke).mock.calls.some(([command])=>command==="undo_world_edit"||command==="delete_station")).toBe(false);
 fireEvent.click(screen.getByRole("tab",{name:"線路"}));await settledMapClick(map,{clientX:250,clientY:130});await settledMapClick(map,{clientX:310,clientY:130});
 replyToCommandOnce("build_plan",{...world,stations:[station],station_placements:[placement],alignments:[{...line,points:[...line.points,point(310,130)]}]});fireEvent.click(screen.getByRole("button",{name:"計画を着工"}));await screen.findByText(/線路を建設しました/);
 fireEvent.click(screen.getByRole("tab",{name:"駅設置"}));const count=vi.mocked(invoke).mock.calls.length;
 fireEvent.keyDown(document.body,{key:"Z",ctrlKey:true,shiftKey:true});await screen.findByText(/元に戻せる操作はありません/);expect(vi.mocked(invoke).mock.calls).toHaveLength(count);expect(map.querySelectorAll(".station-marker")).toHaveLength(1);
});

test('automatic extension rejects a backward join in preview',async()=>{
 const map=await start();
 await settledMapClick(map,{clientX:250,clientY:130});fireEvent.pointerMove(map,{clientX:220,clientY:190});
 expect(await screen.findByText(/線路が折り返しています/)).toBeTruthy();expect(map.querySelector('.invalid-candidate')).toBeTruthy();
 await settledMapClick(map,{clientX:220,clientY:190});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(1);
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(true);
 await settledMapClick(map,{clientX:310,clientY:130});expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
});

test('an alignment used by a service can start and submit an extension',async()=>{
 const map=await start({service_paths:[{service_id:"8",next_leg_key:"2",legs:[{key:"1",segment:{kind:"alignment",alignment_id:"3",from:"4",to:"5"}}],stops:[]}]});
 await settledMapClick(map,{clientX:250,clientY:130});expect(screen.getByText(/から延伸を計画しています/)).toBeTruthy();
 await settledMapClick(map,{clientX:310,clientY:130});vi.mocked(invoke).mockResolvedValueOnce(world);
 fireEvent.click(screen.getByRole("button",{name:"計画を着工"}));await waitFor(()=>expect(invoke).toHaveBeenCalledWith('build_plan',expect.objectContaining({alignments:expect.arrayContaining([expect.objectContaining({extension:{alignmentId:'3',endpoint:'end'}})])})));
});

test('extension inherits non-default design speed and prevents changing its setting',async()=>{
 const map=await start({alignments:[{...line,design_speed_kmh:95}]});
 await settledMapClick(map,{clientX:250,clientY:130});
 const input=screen.getByRole('spinbutton',{name:'線路の目標最高速度'}) as HTMLInputElement;
 expect(input.value).toBe('95');expect(input.disabled).toBe(true);
 await settledMapClick(map,{clientX:310,clientY:133});
 vi.mocked(invoke).mockResolvedValueOnce(world);fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('build_plan',expect.objectContaining({alignments:expect.arrayContaining([expect.objectContaining({designSpeedKmh:95})])})));
});

test('endpoint hover previews the exact clicked extension and clears on leave',async()=>{
 const map=await start();
 fireEvent.pointerMove(map,{clientX:253,clientY:132});
 expect(screen.queryByText(/既存の終点にスナップ/)).toBeNull();
 expect(map.parentElement?.querySelector('.drawing-tooltip')).toBeNull();
 expect(map.querySelector('.endpoint-snap-ring')?.getAttribute('data-allowed')).toBe('true');
 fireEvent.pointerLeave(map);
 expect(map.querySelector('.endpoint-snap-ring')).toBeNull();
 fireEvent.pointerMove(map,{clientX:253,clientY:132});
 await settledMapClick(map,{clientX:253,clientY:132});
 expect(screen.getByText(/延伸元を選択済み：既存の終点（着工前）/)).toBeTruthy();
 expect(map.querySelector('.endpoint-snap-ring')).toBeNull();
 await settledMapClick(map,{clientX:310,clientY:130});
 vi.mocked(invoke).mockResolvedValueOnce(world);
 fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('build_plan',expect.objectContaining({alignments:expect.arrayContaining([expect.objectContaining({extension:{alignmentId:'3',endpoint:'end'}})])})));
});

test('non-owned endpoints show refusal, not a usable connection',async()=>{
 const map=await start({alignments:[{...line,owner:'other'}]});
 fireEvent.pointerMove(map,{clientX:250,clientY:130});
 expect(screen.getByText('他社の線路は延伸できません')).toBeTruthy();
 expect(map.querySelector('.endpoint-snap-ring')?.getAttribute('data-allowed')).toBe('false');
 await settledMapClick(map,{clientX:250,clientY:130});
 expect(screen.queryByText(/延伸元を選択済み/)).toBeNull();
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);
});

test('endpoint outside snap radius has no connection feedback',async()=>{
 const map=await start();
 fireEvent.pointerMove(map,{clientX:260,clientY:130});
 expect(map.querySelector('.endpoint-snap-ring')).toBeNull();
 await settledMapClick(map,{clientX:260,clientY:130});
 expect(screen.queryByText(/延伸元を選択済み/)).toBeNull();
});

test('planned endpoint feedback takes precedence over built endpoints as clicking does',async()=>{
 const map=await start();
 await settledMapClick(map,{clientX:340,clientY:220});
 await settledMapClick(map,{clientX:400,clientY:220});
 await settledMapClick(map,{clientX:400,clientY:220});
 fireEvent.pointerMove(map,{clientX:402,clientY:221});
 expect(screen.queryByText(/計画の終点にスナップ/)).toBeNull();
 expect(map.querySelector(".endpoint-snap-ring")).toBeTruthy();
 await settledMapClick(map,{clientX:402,clientY:221});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 expect(screen.queryByText(/延伸元を選択済み/)).toBeNull();
 await settledMapClick(map,{clientX:440,clientY:220});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(3);
});

for(const endpoint of ['start','end'] as const)test(`tangent guide follows built ${endpoint} extension direction`,async()=>{
 const map=await start();
 await settledMapClick(map,{clientX:endpoint==='start'?180:250,clientY:130});
 const guide=map.querySelector('.construction-tangent-guide')!;
 expect(guide).toBeTruthy();
 const delta=Number(guide.getAttribute('x2'))-Number(guide.getAttribute('x1'));
 expect(endpoint==='start'?delta<0:delta>0).toBe(true);
 expect(Number(guide.getAttribute('y1'))).toBeCloseTo(Number(guide.getAttribute('y2')),5);
});

 test('empty drawing canvas shows no instructional popup but retains drawing measurements',async()=>{
 const map=await start();
 fireEvent.pointerMove(map,{clientX:320,clientY:240});
 expect(screen.queryByText('始点をクリック')).toBeNull();
 expect(map.parentElement?.querySelector('.drawing-tooltip')).toBeNull();
 await settledMapClick(map,{clientX:320,clientY:240});
 fireEvent.pointerMove(map,{clientX:430,clientY:240});
 expect(map.parentElement?.querySelector('.drawing-tooltip')?.textContent).toContain('速度上限');
 expect(screen.queryByText('最後の点をもう一度クリックで終了')).toBeNull();
 });

test('Escape after construction does not expose the internal built line directory',async()=>{
 const map=await start({alignments:[{...line,name:'新しい線路'}]});
 expect(screen.queryByRole('button',{name:'名前未設定の線路'})).toBeNull();
 const css=document.createElement('style');css.textContent=styles;document.head.append(css);
 expect(document.querySelector('.construction-list')).toBeNull();
 fireEvent.keyDown(window,{key:'Escape'});
 expect(screen.queryByRole('button',{name:'名前未設定の線路'})).toBeNull();
 expect(document.querySelector('.construction-list')).toBeNull();
 // Explicit map selection is still available without an internal-object directory.
 fireEvent.click(map.querySelector('.alignment-hit')!);
 expect(screen.queryByText('出典記録なし')).toBeNull();
});

test('built track midpoint hover is visual only and matches endpoint marker size',async()=>{
 const map=await start(),hit=map.querySelector('.alignment-hit')!;
 fireEvent.pointerEnter(hit,{clientX:210,clientY:130});
 const marker=map.querySelector('.built-hover-point')!;
 expect(marker.getAttribute('data-kind')).toBe('line');const radius=marker.getAttribute('r');
 expect(marker.parentElement?.getAttribute('pointer-events')).toBe('none');
 expect(map.querySelector('.endpoint-snap-ring')).toBeNull();
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);
 expect(vi.mocked(invoke).mock.calls.some(([command])=>/^(build|place|extend|add)_/.test(command))).toBe(false);
 fireEvent.pointerMove(hit,{clientX:250,clientY:130});
 expect(map.querySelector('.built-hover-point')).toBeNull();
 expect(map.querySelector('.endpoint-snap-ring')?.getAttribute('r')).toBe(radius);
 fireEvent.pointerLeave(hit);fireEvent.pointerLeave(map);
 expect(map.querySelector('.built-hover-point')).toBeNull();
});

test('inactive hover selects the nearest endpoint when both endpoints are nearby',async()=>{
 const map=await start({alignments:[{...line,points:[point(180,130),point(183,130)]}]});
 fireEvent.keyDown(window,{key:'Escape'});
 const hit=map.querySelector('.alignment-hit')!;
 fireEvent.pointerEnter(hit,{clientX:183,clientY:130});
 const marker=map.querySelector('.built-hover-point')!;
 expect(marker).toBeTruthy();
 expect(marker.getAttribute('data-kind')).toBe('anchor');
 const endPath=hit.getAttribute('d')!.match(/-?\d+(?:\.\d+)?/g)!;
 expect(Number(marker.getAttribute('cx'))).toBeCloseTo(Number(endPath[endPath.length-2]),3);
});

for(const sourceEnd of ['start','end'] as const)for(const targetEnd of ['start','end'] as const)test(`joins built ${sourceEnd} to ${targetEnd} with Ctrl held, exact endpoint and undoable target`,async()=>{
 const x=sourceEnd==='end'?350:100,far=sourceEnd==='end'?420:40;
 const target={...line,id:'4',name:'接続先',design_speed_kmh:80,points:targetEnd==='start'?[point(x,130),point(far,130)]:[point(far,130),point(x,130)]};
 const map=await start({alignments:[line,target]});
 await settledMapClick(map,{clientX:sourceEnd==='end'?250:180,clientY:130});
 fireEvent.pointerMove(map,{clientX:x+2,clientY:131,ctrlKey:true});
 expect(map.querySelector('.destination-snap-ring')?.getAttribute('data-allowed')).toBe('true');
 const hit=map.querySelector('[data-alignment-id="4"] .alignment-hit')!;
 fireEvent.pointerDown(hit,{clientX:x+2,clientY:131,ctrlKey:true,pointerId:1,button:0,isPrimary:true});
 fireEvent.pointerUp(map,{clientX:x+2,clientY:131,ctrlKey:true,pointerId:1,button:0,isPrimary:true});
 await settledMapClick(hit,{clientX:x+2,clientY:131,ctrlKey:true});
 // A follow-up click must not accidentally detach the planned endpoint link.
 await settledMapClick(hit,{clientX:x+2,clientY:131,ctrlKey:true,detail:1});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 vi.mocked(invoke).mockRejectedValueOnce(Error('late target changed'));
 fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));await screen.findByText(/late target changed/);
 const request=vi.mocked(invoke).mock.calls.find(([c])=>c==='build_plan')![1] as any;
 expect(request.alignments[0].destination).toEqual({alignmentId:'4',endpoint:targetEnd});
 expect(request.alignments[0].points.at(-1)).toEqual(point(x,130));
 expect(request.alignments[0].designSpeedKmh).toBe(120);
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
});

test('destination hover preserves ordinary rail/HS type mismatch refusal',async()=>{
 const target={...line,id:'4',track_type:'high_speed',points:[point(350,130),point(420,130)]};
 const map=await start({alignments:[line,target]});
 await settledMapClick(map,{clientX:250,clientY:130});
 fireEvent.pointerMove(map,{clientX:350,clientY:130});
 expect(map.querySelector('.destination-snap-ring')?.getAttribute('data-allowed')).toBe('false');
 await settledMapClick(map,{clientX:350,clientY:130});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(1);
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(true);
});

test.each([['normal','tram'],['tram','high_speed']] as const)('a %s draft connects to a built %s endpoint without changing the new span type',async(sourceType,targetType)=>{
 const source={...line,track_type:sourceType};
 const target={...line,id:'4',track_type:targetType,points:[point(350,130),point(420,130)]};
 const map=await start({alignments:[source,target]});
 await settledMapClick(map,{clientX:250,clientY:130});
 fireEvent.pointerMove(map,{clientX:352,clientY:131});
 expect(map.querySelector('.destination-snap-ring')?.getAttribute('data-allowed')).toBe('true');
 await settledMapClick(map,{clientX:352,clientY:131});
 replyToCommandOnce('build_plan',world);
 fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('build_plan',expect.objectContaining({alignments:[expect.objectContaining({
  trackType:sourceType,extension:{alignmentId:'3',endpoint:'end'},destination:{alignmentId:'4',endpoint:'start'},
 })]})));
 const sent=vi.mocked(invoke).mock.calls.find(([cmd])=>cmd==='build_plan')?.[1] as {alignments:{points:ReturnType<typeof point>[]}[]};
 expect(sent.alignments[0].points[0]).toEqual(source.points[1]);
 expect(sent.alignments[0].points.at(-1)).toEqual(target.points[0]);
});

test('legacy undone destination is recovered on load without creating session history',async()=>{
 const target={...line,id:'4',points:[point(350,130),point(420,130)]};
 const plans={version:1,plans:[{id:1,name:''}],parkedPaths:[],activePathId:1,activePlanId:1,draftPoints:[point(250,130)],redoPoints:[point(350,130)],draftExtension:{alignmentId:'3',endpoint:'end'},draftRedoDestination:{alignmentId:'4',endpoint:'start'},trackType:'normal',designSpeedKmh:120,roadFollowing:false,plannedStations:[],draftFinished:true};
 const map=await start({alignments:[line,target],construction_plans:plans});
 // Legacy redo geometry is recovered on load, not replayed as a session edit.
 await act(async()=>{});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 const restored=map.querySelector('.confirmed-draft')?.getAttribute('d');
 fireEvent.keyDown(document.body,{key:'z',ctrlKey:true});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 expect(map.querySelector('.confirmed-draft')?.getAttribute('d')).toBe(restored);
 fireEvent.keyDown(document.body,{key:'Z',ctrlKey:true,shiftKey:true});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(2);
 expect(map.querySelector('.confirmed-draft')?.getAttribute('d')).toBe(restored);
 expect(vi.mocked(invoke).mock.calls.some(([command])=>command==='undo_world_edit'||command==='redo_world_edit')).toBe(false);
 replyToCommandOnce('build_plan',world);
 fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('build_plan',expect.objectContaining({alignments:expect.arrayContaining([expect.objectContaining({destination:{alignmentId:'4',endpoint:'start'}})])})));
});

test('opposite endpoint of the same built alignment snaps and submits a real ring closure',async()=>{
 const ring={...line,points:[point(350,130),point(420,130),point(420,210),point(180,210),point(180,130),point(250,130)]};
 const map=await start({alignments:[ring]});
 await settledMapClick(map,{clientX:250,clientY:130});
 fireEvent.pointerMove(map,{clientX:352,clientY:131});
 expect(map.querySelector('.destination-snap-ring')?.getAttribute('data-allowed')).toBe('true');
 await settledMapClick(map,{clientX:352,clientY:131});
 vi.mocked(invoke).mockRejectedValueOnce(Error('retain test plan'));
 fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));
 await screen.findByText(/retain test plan/);
 const request=vi.mocked(invoke).mock.calls.find(([c])=>c==='build_plan')![1] as any;
 expect(request.alignments[0].extension).toEqual({alignmentId:'3',endpoint:'end'});
 expect(request.alignments[0].destination).toEqual({alignmentId:'3',endpoint:'start'});
 expect(request.alignments[0].points.at(-1)).toEqual(ring.points[0]);
});

for (const endpoint of ['start','end'] as const) test(`owned connected ${endpoint} reports an existing connection, not foreign ownership`,async()=>{
 const x=endpoint==='start'?180:250;
 const adjacent={...line,id:'4',points:[point(x,130),point(endpoint==='start'?120:310,130)]};
 const map=await start({alignments:[line,adjacent],alignment_connections:[{a:{alignment_id:'3',endpoint},b:{alignment_id:'4',endpoint:'start'}}]});
 fireEvent.pointerMove(map,{clientX:x,clientY:130});
 expect(screen.getByText('この端点は接続済みです')).toBeTruthy();
 expect(screen.queryByText('他社の線路は延伸できません')).toBeNull();
 expect(map.querySelector('.endpoint-snap-ring')?.getAttribute('data-allowed')).toBe('false');
 await settledMapClick(map,{clientX:x,clientY:130});
 expect(map.querySelectorAll('.draft-anchor')).toHaveLength(0);
 expect(screen.queryByText(/延伸元を選択済み/)).toBeNull();
});

test('reconnected split pieces render a shared D coordinate and report its connection',async()=>{
 const left={...line,points:[point(100,130),point(180,130),point(250,130)],anchor_indices:[0,1,2]};
 const right={...line,id:'4',points:[point(250,130),point(330,130)],anchor_indices:[0,1]};
 const map=await start({alignments:[left,right],alignment_connections:[{a:{alignment_id:'3',endpoint:'end'},b:{alignment_id:'4',endpoint:'start'}}]});
 const leftPath=map.querySelector('[data-alignment-id="3"] .alignment-path')!.getAttribute('d')!;
 const rightPath=map.querySelector('[data-alignment-id="4"] .alignment-path')!.getAttribute('d')!;
 expect(leftPath.split(' ').at(-1)!.slice(1)).toBe(rightPath.split(' ')[0].slice(1));
 fireEvent.pointerMove(map,{clientX:250,clientY:130});
 expect(screen.getByText('この端点は接続済みです')).toBeTruthy();
 expect(screen.queryByText('他社の線路は延伸できません')).toBeNull();
});


test.each([
 ['normal','トラム','tram',55],
 ['high_speed','トラム','tram',55],
 ['tram','通常','normal',115],
 ['tram','高速','high_speed',270],
] as const)('a deliberate %s → %s choice survives built start snapping and stays locked',async(sourceType,label,newType,speed)=>{
 const source={...line,track_type:sourceType,design_speed_kmh:85};
 const before=structuredClone(source);
 const map=await start({alignments:[source]});
 const choice=screen.getByRole('button',{name:label});
 fireEvent.click(choice);
 const input=screen.getByRole('spinbutton',{name:'線路の目標最高速度'}) as HTMLInputElement;
 fireEvent.change(input,{target:{value:String(speed)}});
 if(newType==='tram')fireEvent.click(screen.getByRole('button',{name:'道路に合わせる'}));
 await settledMapClick(map,{clientX:253,clientY:132});
 expect(choice.getAttribute('aria-pressed')).toBe('true');
 expect(choice.matches(':disabled')).toBe(true);
 expect(input.value).toBe(String(speed));
 expect(input.disabled).toBe(true);
 await settledMapClick(map,{clientX:310,clientY:130});
 const expected=expect.objectContaining({
   trackType:newType,designSpeedKmh:speed,extension:{alignmentId:'3',endpoint:'end'},points:[source.points[1],point(310,130)],
 });
 expect(invoke).toHaveBeenCalledWith('preview_construction_plan',expect.objectContaining({validationOnly:true,alignments:[expected]}));
 replyToCommandOnce('build_plan',world);
 fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith('build_plan',expect.objectContaining({alignments:[expected]})));
 expect(source).toEqual(before);
});

test('snapping with no toolbar choice still inherits a built tram and its speed',async()=>{
 const source={...line,track_type:'tram',design_speed_kmh:38};
 const map=await start({alignments:[source]});
 await settledMapClick(map,{clientX:250,clientY:130});
 expect(screen.getByRole('button',{name:'トラム'}).getAttribute('aria-pressed')).toBe('true');
 expect((screen.getByRole('spinbutton',{name:'線路の目標最高速度'}) as HTMLInputElement).value).toBe('38');
});

test('explicit same-family rail selection still inherits source type and speed',async()=>{
 const map=await start({alignments:[{...line,design_speed_kmh:85}]});
 fireEvent.click(screen.getByRole('button',{name:'高速'}));
 await settledMapClick(map,{clientX:250,clientY:130});
 expect(screen.getByRole('button',{name:'通常'}).getAttribute('aria-pressed')).toBe('true');
 expect((screen.getByRole('spinbutton',{name:'線路の目標最高速度'}) as HTMLInputElement).value).toBe('85');
});
