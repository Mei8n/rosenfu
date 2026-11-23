// @vitest-environment jsdom
import React from 'react';
import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
import {App} from './App';import {invoke} from '@tauri-apps/api/core';import {project} from './map';
import {createRoadDrawingEngine} from './roadDrawingEngine';
import {alignmentLength} from './alignment';
import {sampledGeometry} from './editedGeometry';
import {provisionalCurveSpeed} from './circularAlignment';
import {createOpenFreeMapRoadRepository,decodeOpenFreeMapRoadTile,OFM_ATTRIBUTION,OFM_EVIDENCE_RULE,OFM_METADATA} from './openFreeMapRoadData';
import tramFixture from '../crates/rosenfu-core/tests/fixtures/ofm_authored_tram_synthetic.json';
const mock=vi.hoisted(()=>({snapshot:null as any,read:vi.fn(),observe:vi.fn()}));
vi.mock('./useAutomaticRoad',()=>({useAutomaticRoad:()=>({status:'ready',snapshot:mock.snapshot,message:'fixture',retry:vi.fn(),ensureCurrent:vi.fn(async()=>({status:'unavailable',reason:'fixture'})),readTileEvidenceBatch:mock.read,observeRouteTiles:mock.observe})}));
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:()=>true}));
class RoadWorker {
 static delay=false;static queued:(()=>void)[]=[];
 onmessage:any;onerror:any;engine:any;closed=false;
 postMessage(request:any){if(request.kind==='load'){this.engine=createRoadDrawingEngine(request.bundle,request.tileLines);return;}void this.engine(request.query).then((result:any)=>{const deliver=()=>{if(!this.closed)this.onmessage?.({data:{id:request.id,result}});};if(RoadWorker.delay)RoadWorker.queued.push(deliver);else deliver();});}
 terminate(){this.closed=true;}
}
afterEach(()=>{cleanup();vi.clearAllMocks();vi.unstubAllGlobals();localStorage.clear();});
const longitude=139.76806640625,latitude=35.684,camera={latitude,longitude,zoom:16};
const world={view_state:camera,next_id:'3',players:[{id:'1',name:'P'}],companies:[{id:'2',owner:'1',name:'C'}],stations:[],tracks:[],routes:[],services:[],alignments:[],station_placements:[]};
async function start(snapshot:any=world,vertices?:{longitude:number;latitude:number}[],openRoad=true,includeIneligible=false){
 RoadWorker.delay=false;RoadWorker.queued=[];vi.stubGlobal('Worker',RoadWorker);
 const bundle={graph:{format:'rosenfu-reference-graph',version:1,dataset:{id:'t',revision:'r',source:'t',license:'t',bounds:{west:139,east:140,south:35,north:36}},nodes:[{osmId:'1',longitude:longitude-.002,latitude},{osmId:'2',longitude:longitude+.002,latitude}],edges:[{id:'1',osmWayId:'1',segmentIndex:0,from:'1',to:'2',mode:'road',layer:0,level:null,bridge:false,tunnel:false}]},roadWays:{'1':{highway:'residential',access:null,motorVehicle:null,structureUnsupported:false}},graphSha256:'a'.repeat(64),sourceSha256:'b'.repeat(64),source:{source:'t',sourceDate:null,license:'t',attribution:'t'}};
 if(vertices){bundle.graph.nodes=vertices.map((p,i)=>({...p,osmId:String(i+1)}));bundle.graph.edges=vertices.slice(1).map((_,i)=>({id:String(i+1),osmWayId:'1',segmentIndex:i,from:String(i+1),to:String(i+2),mode:'road',layer:0,level:null,bridge:false,tunnel:false}));}
 if(includeIneligible){
  bundle.graph.nodes.push({osmId:'90',longitude:longitude-.002,latitude:latitude+.0003},{osmId:'91',longitude:longitude+.002,latitude:latitude+.0003});
  for(const id of ['90','91'])bundle.graph.edges.push({id,osmWayId:id,segmentIndex:0,from:'90',to:'91',mode:id==='90'?'road':'rail',layer:0,level:null,bridge:false,tunnel:false});
  Object.assign(bundle.roadWays,{'90':{highway:'residential',access:'private',motorVehicle:null,structureUnsupported:false}});
 }
 mock.snapshot={bundle,roads:[]};
 vi.mocked(invoke).mockImplementation(async(command)=>{if(command==='engine_info')return {name:'Rosenfu',version:'road-app'};if(command==='list_save_slots')return {active_slot_id:'slot',slots:[]};if(command==='autosave_status')return {available:false};if(command==='save_world')return {saved:true};if(command==='preview_construction_plan')return {validationError:null,newCrossingCount:0};return snapshot;});
 render(<App/>);await screen.findByText(/road-app/);if(openRoad)fireEvent.click(screen.getByRole('button',{name:'道路に合わせる'}));
 const map=document.querySelector('svg[role=img]')!;map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:1000,bottom:600,width:1000,height:600,toJSON:()=>({})});
 const click=async(offset:number,ctrlKey=false,detail=1)=>{const p=project(latitude,longitude+offset,camera);await act(async()=>{fireEvent.click(map,{clientX:p.x,clientY:p.y,ctrlKey,detail});await new Promise(r=>setTimeout(r,0));});};return {map,click};
}
test('road points form an editable continuous draft; undo and redo restore the exact road geometry',async()=>{
 const {click}=await start();await click(-.0008);await click(0);await click(.0008);
 const path=()=>document.querySelector('.confirmed-draft')?.getAttribute('d');const before=path();expect(before).toBeTruthy();expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
 fireEvent.keyDown(window,{key:'z',ctrlKey:true});expect(path()).not.toBe(before);fireEvent.keyDown(window,{key:'z',ctrlKey:true,shiftKey:true});expect(path()).toBe(before);
 fireEvent.keyDown(window,{key:'Escape'});expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
});
test('Ctrl temporarily uses ordinary placement without changing the road toggle',async()=>{const {click}=await start();await click(-.0008);await click(.0008,true);expect(document.querySelector('.confirmed-draft')).toBeTruthy();expect(screen.getByRole('button',{name:'道路に合わせる'}).getAttribute('aria-pressed')).toBe('true');expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);});

test('save stores normal editable geometry, reload retains it and permits continuation',async()=>{
 const h=await start();await h.click(-.0008);await h.click(0);const before=document.querySelector('.confirmed-draft')?.getAttribute('d');
 fireEvent.click(screen.getByRole('tab',{name:'セーブデータ'}));fireEvent.click(screen.getByRole('button',{name:'ワールドを保存'}));
 await waitFor(()=>expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='save_world')).toBe(true));
 const plans=(vi.mocked(invoke).mock.calls.find(([c])=>c==='save_world')![1] as any).constructionPlans;
 expect(plans.roadFollowing).toBe(false);expect(plans.draftEditedGeometry.line.length).toBeGreaterThan(1);expect(JSON.stringify(plans)).not.toContain('rawEvidence');
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='stage_ofm_construction_plan')).toBe(false);
 cleanup();const reloaded=await start({...world,construction_plans:plans});expect(document.querySelector('.confirmed-draft')?.getAttribute('d')).toBe(before);await reloaded.click(.0008);expect(document.querySelector('.confirmed-draft')?.getAttribute('d')).not.toBe(before);
});

test('road continuation connects exact built endpoints and preserves their original geometry',async()=>{
 const a={latitude,longitude:longitude-.0008},b={latitude,longitude:longitude+.0008};
 const source={id:'10',owner:'2',name:'source',points:[{latitude,longitude:longitude-.0015},a]},target={id:'11',owner:'2',name:'target',points:[b,{latitude,longitude:longitude+.0015}]};
 const snapshot={...world,alignments:[source,target]};const h=await start(snapshot);await h.click(-.0008);await h.click(.0008);
 expect(screen.getByRole('button',{name:'道路に合わせる'}).getAttribute('aria-pressed')).toBe('true');expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
 fireEvent.click(screen.getByRole('tab',{name:'セーブデータ'}));fireEvent.click(screen.getByRole('button',{name:'ワールドを保存'}));await waitFor(()=>expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='save_world')).toBe(true));
 const plans=(vi.mocked(invoke).mock.calls.find(([c])=>c==='save_world')![1] as any).constructionPlans;expect(plans.draftExtension).toEqual({alignmentId:'10',endpoint:'end'});expect(plans.draftDestination).toEqual({alignmentId:'11',endpoint:'start'});expect(plans.draftPoints[0]).toEqual(a);expect(plans.draftPoints.at(-1)).toEqual(b);expect(source.points[1]).toEqual(a);expect(target.points[0]).toEqual(b);
});

test('native refusal and transport failure never append the road anchor',async()=>{
 const h=await start();await h.click(-.0008);const base=vi.mocked(invoke).getMockImplementation()!;
 vi.mocked(invoke).mockImplementation(async(c,a)=>c==='preview_construction_plan'?{validationError:'water refusal'}:base(c,a));await h.click(.0008);expect(document.querySelector('.confirmed-draft')).toBeNull();expect(screen.getByText(/water refusal/)).toBeTruthy();
 vi.mocked(invoke).mockImplementation(async(c,a)=>{if(c==='preview_construction_plan')throw Error('network');return base(c,a);});await h.click(.0008);expect(document.querySelector('.confirmed-draft')).toBeNull();expect(screen.getByText(/敷設判定を取得できません/)).toBeTruthy();
 vi.mocked(invoke).mockImplementation(base);await h.click(.0008);expect(document.querySelector('.confirmed-draft')).toBeTruthy();
});

test('Escape during exact native validation discards the delayed placement',async()=>{
 const h=await start();await h.click(-.0008);const base=vi.mocked(invoke).getMockImplementation()!;let release!:(value:any)=>void;
 vi.mocked(invoke).mockImplementation(async(c,a)=>c==='preview_construction_plan'&&a&&'roads' in a?new Promise(resolve=>{release=resolve;}):base(c,a));await h.click(.0008);await waitFor(()=>expect(release).toBeTruthy());
 fireEvent.keyDown(window,{key:'Escape'});await act(async()=>release({validationError:null}));expect(document.querySelector('.confirmed-draft')).toBeNull();
});
test('double click during worker calculation adds the final road segment then finishes',async()=>{
 const h=await start();await h.click(-.0008);RoadWorker.delay=true;await h.click(0);await h.click(0,false,2);
 expect(document.querySelector('.confirmed-draft')).toBeNull();RoadWorker.delay=false;await act(async()=>{RoadWorker.queued.splice(0).forEach(deliver=>deliver());await new Promise(r=>setTimeout(r,0));});
 expect(document.querySelector('.confirmed-draft')).toBeTruthy();await h.click(.0008);expect(document.querySelector('.parked-plan')).toBeTruthy();expect(document.querySelector('.confirmed-draft')).toBeNull();
});
test('speed change while the worker is pending invalidates its captured placement intent',async()=>{
 const h=await start();await h.click(-.0008);RoadWorker.delay=true;await h.click(.0008);expect(RoadWorker.queued).toHaveLength(1);
 fireEvent.change(screen.getByLabelText('線路の目標最高速度'),{target:{value:'180'}});
 RoadWorker.delay=false;await act(async()=>{RoadWorker.queued.splice(0).forEach(deliver=>deliver());await new Promise(r=>setTimeout(r,0));});
 expect(document.querySelector('.confirmed-draft')).toBeNull();expect((screen.getByLabelText('線路の目標最高速度') as HTMLInputElement).value).toBe('180');
 await h.click(.0008);expect(document.querySelector('.confirmed-draft')).toBeTruthy();
});
test('Ctrl and off-road fallback cannot bypass the native placement guard',async()=>{
 const h=await start();await h.click(-.0008);const base=vi.mocked(invoke).getMockImplementation()!;
 vi.mocked(invoke).mockImplementation(async(c,a)=>c==='preview_construction_plan'?{validationError:'placement refused'}:base(c,a));
 await h.click(.0008,true);expect(document.querySelector('.confirmed-draft')).toBeNull();
 const p=project(latitude+.001,longitude+.0008,camera);await act(async()=>{fireEvent.click(h.map,{clientX:p.x,clientY:p.y});await new Promise(r=>setTimeout(r,0));});expect(document.querySelector('.confirmed-draft')).toBeNull();expect(screen.getByText(/placement refused/)).toBeTruthy();
});
test('legacy OFM sources keep provider attribution even without drawn alignments',async()=>{
 await start({...world,source_records:[{dataset_id:'ofm-tiles/legacy'}]});fireEvent.click(screen.getByRole('button',{name:'道路に合わせる'}));
 expect(screen.getByRole('link',{name:'OpenFreeMap'})).toBeTruthy();expect(screen.getByRole('link',{name:'© OpenMapTiles'})).toBeTruthy();
});

test('road preview suppresses the unrelated free chord and Ctrl restores ordinary preview',async()=>{
 vi.stubGlobal('PointerEvent',MouseEvent);const {map,click}=await start();await click(-.0008);const p=project(latitude,longitude+.0008,camera);
 await act(async()=>{fireEvent.pointerMove(map,{clientX:p.x,clientY:p.y});await new Promise(r=>setTimeout(r,0));});
 await waitFor(()=>expect(document.querySelector('.road-drawing-preview')).toBeTruthy());expect(document.querySelector('.tram-road-guide-overlay,.tram-road-guide-preview,.tram-road-guide-marker')).toBeNull();expect(document.querySelector('.reference-graph-road')?.getAttribute('d')).toMatch(/^M/);expect(document.querySelector('.candidate-draft')).toBeNull();expect(document.querySelector('.curve-circle-guide')).toBeNull();
 await act(async()=>{fireEvent.pointerMove(map,{clientX:p.x,clientY:p.y,ctrlKey:true});});expect(document.querySelector('.road-drawing-preview')).toBeNull();expect(document.querySelector('.candidate-draft')).toBeTruthy();
});

test('curved road preview reports routed length and curve speed rather than the free chord',async()=>{
 vi.stubGlobal('PointerEvent',MouseEvent);const vertices=[{longitude:longitude-.001,latitude},{longitude,latitude},{longitude,latitude:latitude+.001}];const {map,click}=await start(world,vertices);await click(-.0008);
 const point={longitude,latitude:latitude+.0008},pixel=project(point.latitude,point.longitude,camera);await act(async()=>{fireEvent.pointerMove(map,{clientX:pixel.x,clientY:pixel.y});await new Promise(r=>setTimeout(r,0));});await waitFor(()=>expect(document.querySelector('.road-drawing-preview')).toBeTruthy());
 const expected=await createRoadDrawingEngine(mock.snapshot.bundle,[])({point,start:{longitude:longitude-.0008,latitude},radiusM:20});if(expected.kind!=='route')throw Error('fixture did not route');const geometry=sampledGeometry(expected.line),speed=Math.min(120,...geometry.arcs.map(a=>provisionalCurveSpeed(a.radiusM)??0));
 expect(geometry.arcs.length).toBeGreaterThan(0);expect(document.querySelector('.drawing-metrics')?.textContent).toContain(`＋ ${Math.round(alignmentLength(expected.line))} m`);expect(document.querySelector('.drawing-speed')?.textContent).toBe(`${speed} km/h`);
});
test('unfittable road corner displays its specific refusal instead of adding an anchor',async()=>{
 vi.stubGlobal('PointerEvent',MouseEvent);const vertices=[{longitude:longitude-.0002,latitude},{longitude,latitude},{longitude,latitude:latitude+.001}];const {map,click}=await start(world,vertices);await click(-.0002);const point=project(latitude+.0008,longitude,camera);await act(async()=>{fireEvent.pointerMove(map,{clientX:point.x,clientY:point.y});await new Promise(r=>setTimeout(r,0));});await waitFor(()=>expect(document.querySelector('.drawing-metrics')?.textContent).toContain('カーブをつなぐための道路の長さが足りません'));
 await act(async()=>{fireEvent.click(map,{clientX:point.x,clientY:point.y,detail:1});await new Promise(r=>setTimeout(r,0));});expect(document.querySelectorAll('.draft-anchor')).toHaveLength(1);
});
test('tram Ctrl inverts snap at a stationary cursor, and clicking commits the shown geometry',async()=>{
 vi.stubGlobal('PointerEvent',MouseEvent);
 const vertices=[{longitude:longitude-.001,latitude},{longitude,latitude},{longitude,latitude:latitude+.001}];
 const {map,click}=await start(world,vertices);
 const toggle=screen.getByRole('button',{name:'道路に合わせる'});
 fireEvent.click(screen.getByRole('button',{name:'トラム'}));
 expect(toggle.getAttribute('aria-pressed')).toBe('true');
 fireEvent.click(toggle);
 expect(toggle.getAttribute('aria-pressed')).toBe('false');
 await click(-.0008);
 const end=vertices.at(-1)!;
 const point={longitude:end.longitude,latitude:latitude+.0008},pixel=project(point.latitude,point.longitude,camera);
 const move=()=>fireEvent.pointerMove(map,{clientX:pixel.x,clientY:pixel.y});
 const road=()=>document.querySelector('.road-drawing-preview');
 await act(async()=>move());
 expect(road()).toBeNull();
 fireEvent.keyDown(window,{key:'Control',ctrlKey:true});
 await waitFor(()=>expect(road()).toBeTruthy());
 fireEvent.keyUp(window,{key:'Control',ctrlKey:false});
 expect(road()).toBeNull();
 fireEvent.keyDown(window,{key:'Control',ctrlKey:true});
 await waitFor(()=>expect(road()).toBeTruthy());
 const result=await createRoadDrawingEngine(mock.snapshot.bundle,[])({point,start:{longitude:longitude-.0008,latitude},radiusM:20});
 if(result.kind!=='route')throw Error('fixture did not route');
 const expectedLine=result.line;
 RoadWorker.delay=true;
 await act(async()=>{fireEvent.click(map,{clientX:pixel.x,clientY:pixel.y,detail:1,ctrlKey:true});await new Promise(r=>setTimeout(r,0));});
 expect(RoadWorker.queued.length).toBeGreaterThan(0);
 fireEvent.keyUp(window,{key:'Control',ctrlKey:false});
 expect(road()).toBeNull();
 RoadWorker.delay=false;
 await act(async()=>{RoadWorker.queued.splice(0).forEach(deliver=>deliver());await new Promise(r=>setTimeout(r,0));});
 await waitFor(()=>expect(document.querySelector('.confirmed-draft')).toBeTruthy());
 const reqs=vi.mocked(invoke).mock.calls.filter(([name,args])=>name==='preview_construction_plan'&&(args as any)?.validationOnly);
 const alignment=(reqs.at(-1)?.[1] as any).alignments[0];
 expect(alignment.trackType).toBe('tram');
 expect(alignment.points).toHaveLength(expectedLine.length);
 alignment.points.forEach((p:any,i:number)=>{expect(p.longitude).toBeCloseTo(expectedLine[i].longitude,8);expect(p.latitude).toBeCloseTo(expectedLine[i].latitude,8);});
 expect(toggle.getAttribute('aria-pressed')).toBe('false');
});

test('ordinary Tram starts with a green guide; road off or temporary Ctrl leaves free drawing uncoloured',async()=>{
 vi.stubGlobal('PointerEvent',MouseEvent);
 const {map,click}=await start(world,undefined,false,true);
 const toggle=screen.getByRole('button',{name:'道路に合わせる'});
 const overlay=()=>document.querySelector('.tram-road-guide-overlay');
 const trace=()=>document.querySelector('.road-drawing-preview');
 const marker=()=>document.querySelector('.road-snap-point');
 expect(toggle.getAttribute('aria-pressed')).toBe('false');
 expect(overlay()).toBeNull();
 fireEvent.click(screen.getByRole('button',{name:'トラム'}));
 expect(toggle.getAttribute('aria-pressed')).toBe('true');
 const geometry=(offset:number)=>{
  const a=project(latitude+offset,longitude-.002,camera),b=project(latitude+offset,longitude+.002,camera);
  return `M${a.x},${a.y}L${b.x},${b.y}`;
 };
 expect(overlay()?.getAttribute('d')).toBe(geometry(0));
 expect(document.querySelector('.reference-graph-road:not(.tram-road-guide-overlay)')?.getAttribute('d')).toBe(geometry(.0003));
 expect(document.querySelector('.reference-graph-rail')?.getAttribute('d')).toBe(geometry(.0003));
 await click(-.0008);
 const p=project(latitude,longitude+.0008,camera);
 await act(async()=>fireEvent.pointerMove(map,{clientX:p.x,clientY:p.y}));
 await waitFor(()=>expect(trace()?.classList.contains('tram-road-guide-preview')).toBe(true));
 expect(marker()?.classList.contains('tram-road-guide-marker')).toBe(true);
 fireEvent.keyDown(window,{key:'Control',ctrlKey:true});
 expect(overlay()).toBeNull();
 expect(trace()).toBeNull();
 expect(marker()).toBeNull();
 expect(document.querySelector('.candidate-draft')).toBeTruthy();
 fireEvent.keyUp(window,{key:'Control',ctrlKey:false});
 await waitFor(()=>expect(trace()?.classList.contains('tram-road-guide-preview')).toBe(true));
 expect(overlay()).toBeTruthy();
 fireEvent.keyDown(window,{key:'Control',ctrlKey:true});
 expect(overlay()).toBeNull();
 fireEvent.blur(window);
 expect(trace()).toBeNull();
 await act(async()=>fireEvent.pointerMove(map,{clientX:p.x,clientY:p.y}));
 await waitFor(()=>expect(trace()?.classList.contains('tram-road-guide-preview')).toBe(true));
 fireEvent.click(toggle);
 expect(toggle.getAttribute('aria-pressed')).toBe('false');
 expect(overlay()).toBeNull();
 expect(trace()).toBeNull();
 expect(marker()).toBeNull();
 fireEvent.keyDown(window,{key:'Control',ctrlKey:true});
 await waitFor(()=>expect(overlay()?.getAttribute('d')).toMatch(/^M/));
 await waitFor(()=>expect(trace()?.classList.contains('tram-road-guide-preview')).toBe(true));
 fireEvent.keyUp(window,{key:'Control',ctrlKey:false});
 expect(overlay()).toBeNull();
 expect(trace()).toBeNull();
});

test('a same-position double click finishes temporary tram road snap during native validation',async()=>{
 const {click}=await start();
 fireEvent.click(screen.getByRole('button',{name:'トラム'}));
 const toggle=screen.getByRole('button',{name:'道路に合わせる'});
 fireEvent.click(toggle);
 await click(-.0008);
 fireEvent.keyDown(window,{key:'Control',ctrlKey:true});
 const base=vi.mocked(invoke).getMockImplementation()!;
 let release!:(value:any)=>void;
 vi.mocked(invoke).mockImplementation(async(c,a)=>c==='preview_construction_plan'&&(a as any)?.validationOnly===true
   ?new Promise(resolve=>{release=resolve;})
   :base(c,a));
 await click(0,true);
 await waitFor(()=>expect(release).toBeTruthy());
 expect(document.querySelector('.confirmed-draft')).toBeNull();
 // The accepted click was road-assisted. Releasing Ctrl before the second
 // browser click must not turn the finish intent into an ordinary placement.
 fireEvent.keyUp(window,{key:'Control',ctrlKey:false});
 await click(0,false,2);
 await act(async()=>release({validationError:null,newCrossingCount:0}));
 expect(document.querySelector('.confirmed-draft')).toBeTruthy();
 expect(toggle.getAttribute('aria-pressed')).toBe('false');
 vi.mocked(invoke).mockImplementation(base);
 await click(.0008);
 expect(document.querySelector('.parked-plan')).toBeTruthy();
 expect(document.querySelector('.confirmed-draft')).toBeNull();
});

test('ordinary authored tram follows a cached raw T junction on click without invoking native on hover',async()=>{
 vi.stubGlobal('Worker',RoadWorker);vi.stubGlobal('PointerEvent',MouseEvent);RoadWorker.delay=false;
 const f=tramFixture,bytes=Uint8Array.from(f.raw),tile=await decodeOpenFreeMapRoadTile(bytes,f.x,f.y,f.revision,1);
 mock.snapshot={...tile,tileSnapshots:[tile]};
 const [[startLon,startLat],[jointLon,jointLat]]=tile.tileLines[0].coordinates;
 const [endLon,endLat]=tile.tileLines[1].coordinates[1];
 const start={latitude:startLat+(jointLat-startLat)*.25,longitude:startLon+(jointLon-startLon)*.25};
 const end={latitude:jointLat+(endLat-jointLat)*.75,longitude:jointLon+(endLon-jointLon)*.75};
 const junction={latitude:jointLat,longitude:jointLon};
 const mapCamera={...junction,zoom:16},loaded={...world,view_state:mapCamera};
 const binding={z:14,x:f.x,y:f.y,revision:f.revision,rawSha256:f.tileRawSha256,decoderRule:OFM_EVIDENCE_RULE,
     key:f.tileKey,url:f.tileKey,license:'ODbL-1.0',attribution:OFM_ATTRIBUTION};
 mock.read.mockResolvedValue({status:'ready',evidence:{rawTiles:[{sha256:f.tileRawSha256,bytes}],bindings:[binding]}});
 let allowReply!:(points:typeof start[])=>void,denyReply!:(error:Error)=>void;
 vi.mocked(invoke).mockImplementation(async(command)=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'tram-junction',authored_tram_road_guide_enabled:true};
  if(command==='list_save_slots')return {active_slot_id:'slot',slots:[]};
  if(command==='autosave_status')return {available:false};
  if(command==='ofm_world_token')return 'tram-junction-test';
  if(command==='preview_authored_tram_road_guide')return new Promise((resolve,reject)=>{allowReply=resolve;denyReply=reject;});
  if(command==='preview_construction_plan')return {validationError:null,newCrossingCount:0};
  return loaded;
 });
 const {rerender}=render(<App/>);await screen.findByText(/tram-junction/);
 fireEvent.click(screen.getByRole('button',{name:'トラム'}));
 expect(screen.getByRole('button',{name:'道路に合わせる'}).getAttribute('aria-pressed')).toBe('true');
 const map=document.querySelector('svg[role=img]')!;map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:1000,bottom:600,width:1000,height:600,toJSON:()=>({})});
 const gesture=(p:typeof start)=>{const xy=project(p.latitude,p.longitude,mapCamera);return {clientX:xy.x,clientY:xy.y,detail:1};};
 await act(async()=>{fireEvent.click(map,gesture(start));await new Promise(r=>setTimeout(r,0));});
 await act(async()=>{fireEvent.pointerMove(map,gesture(end));await new Promise(r=>setTimeout(r,0));});
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='preview_authored_tram_road_guide')).toBe(false);
 expect(mock.read).not.toHaveBeenCalled();
 await waitFor(()=>expect(document.querySelector('.road-snap-point')).toBeTruthy());
 expect(document.querySelector('.candidate-draft')).toBeNull();
 await act(async()=>{fireEvent.click(map,gesture(end));await new Promise(r=>setTimeout(r,0));});
 await waitFor(()=>expect(allowReply).toBeTruthy());
 expect(document.querySelector('.confirmed-draft')).toBeNull();
 const call=vi.mocked(invoke).mock.calls.find(([c])=>c==='preview_authored_tram_road_guide')![1] as any;
 expect(call.guide).toEqual({tileKey:f.tileKey,tileRawSha256:f.tileRawSha256,start:{feature:0,line:0,vertex:0},end:{feature:1,line:0,vertex:0},startFractionQ:250000,endFractionQ:750000});
 expect(call.expectedWorldToken).toBe('tram-junction-test');
 expect(call.rawEvidence.sha256).toBe(f.tileRawSha256);
 // A late result after toggling the mode must never place the road leg.
 const firstReply=allowReply;
 const toggle=screen.getByRole('button',{name:'道路に合わせる'});
 fireEvent.click(toggle);
 await act(async()=>firstReply([start,junction,end]));
 expect(document.querySelector('.confirmed-draft')).toBeNull();
 fireEvent.click(toggle);
 await act(async()=>{fireEvent.click(map,gesture(end));await new Promise(r=>setTimeout(r,0));});
 await waitFor(()=>expect(allowReply).not.toBe(firstReply));
 const staleSourceReply=allowReply;
 const oldSnapshot=mock.snapshot;
 mock.snapshot={...oldSnapshot};rerender(<App/>);
 await act(async()=>staleSourceReply([start,junction,end]));
 expect(document.querySelector('.confirmed-draft')).toBeNull();
 await act(async()=>{fireEvent.click(map,gesture(end));await new Promise(r=>setTimeout(r,0));});
 await waitFor(()=>expect(allowReply).not.toBe(staleSourceReply));
 const refusedReply=allowReply;
 await act(async()=>{fireEvent.click(map,{...gesture(end),detail:2});denyReply(Error('接続が証明できません'));});
 await screen.findByText(/接続が証明できません/);
 expect(document.querySelector('.confirmed-draft')).toBeNull(); // hard denial: no fallback
 await act(async()=>{fireEvent.click(map,gesture(end));await new Promise(r=>setTimeout(r,0));});
 await waitFor(()=>expect(allowReply).not.toBe(refusedReply));
 await act(async()=>allowReply([start,junction,end]));
 await waitFor(()=>expect(document.querySelector('.confirmed-draft')).toBeTruthy());
 const reqs=vi.mocked(invoke).mock.calls.filter(([c,args])=>c==='preview_construction_plan'&&(args as any)?.validationOnly);
 const built=(reqs.at(-1)![1] as any).alignments[0];
 expect(built.trackType).toBe('tram');
 expect(built.points.length).toBeGreaterThan(3);
 expect(built.points[0].longitude).toBeCloseTo(start.longitude,8);
 expect(built.points[0].latitude).toBeCloseTo(start.latitude,8);
 expect(built.points.at(-1).longitude).toBeCloseTo(end.longitude,8);
 expect(built.points.at(-1).latitude).toBeCloseTo(end.latitude,8);
 expect(built.ofmPath).toBeUndefined();
 // The rejected double click didn't leave an old 'finish' request.
 const fresh={latitude:jointLat+(endLat-jointLat)*.95,longitude:endLon};
 await act(async()=>{fireEvent.click(map,gesture(fresh));await new Promise(r=>setTimeout(r,0));});
 expect(document.querySelector('.parked-plan')).toBeNull();
});

test.each(['confirmed-road-snap','free-on-road','undo-road-snap'] as const)('Tram missing old cell %s accepts only matching road snap proof',async(kind)=>{
 vi.stubGlobal('Worker',RoadWorker);vi.stubGlobal('PointerEvent',MouseEvent);RoadWorker.delay=false;
 const revision='rosenfu_webview_fixture_16';
 const tile=(x:number)=>`${OFM_METADATA}/${revision}/14/${x}/6452.pbf`;
 const start={latitude:35.6740144226818,longitude:139.74504232406616};
 const end={latitude:35.67362222198714,longitude:139.75226283073425};
 const left='GkoKDnRyYW5zcG9ydGF0aW9ueAIogCASFxIEAAABARgCIg0J+DyIBBKMAbQBzAMAGgVjbGFzcyIHCgVtaW5vchoGb25ld2F5IgIgAQ==';
 const right='GkYKDnRyYW5zcG9ydGF0aW9ueAIogCASExIEAAABARgCIgkJzwG8BQrMEwAaBWNsYXNzIgcKBW1pbm9yGgZvbmV3YXkiAiAB';
 const fetcher=vi.fn(async(url:unknown)=>url===OFM_METADATA?
  Response.json({maxzoom:14,tiles:[`${OFM_METADATA}/${revision}/{z}/{x}/{y}.pbf`]}):
  url===tile(14551)||url===tile(14552)?new Response(Uint8Array.from(atob(url===tile(14551)?left:right),c=>c.charCodeAt(0))):new Response(null,{status:404}));
 const repo=createOpenFreeMapRoadRepository(fetcher);
 const around=({latitude,longitude}:typeof start)=>({west:longitude-.0001,east:longitude+.0001,south:latitude-.0001,north:latitude+.0001});
 mock.snapshot=await repo.load(around(start));
 const startTile=mock.snapshot;
 const endTile=await repo.load(around(end));
 let observeWaiting:undefined|(()=>void);
 mock.observe.mockImplementation(async(...args:Parameters<typeof repo.observeTiles>)=>{
  const refs=await repo.observeTiles(...args);
  if(observeWaiting===undefined)await new Promise<void>(resolve=>{observeWaiting=resolve;});
  return refs;
 });
 mock.read.mockImplementation(async(...args:Parameters<typeof repo.readTileEvidenceBatch>)=>({status:'ready',evidence:await repo.readTileEvidenceBatch(...args)}));
 const mapCamera={latitude:35.6738,longitude:139.748,zoom:16};
 const loaded={...world,view_state:mapCamera};
 let nativeReject=true;
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command==='engine_info')return {name:'Rosenfu',version:'tram-rescope',authored_tram_road_guide_enabled:true};
  if(command==='list_save_slots')return {active_slot_id:'slot',slots:[]};
  if(command==='autosave_status')return {available:false};
  if(command==='preview_construction_plan'&&(args as {validationOnly?:boolean})?.validationOnly===true)
    return {validationError:nativeReject?'試験の勾配拒否':null,newCrossingCount:0};
  if(command==='preview_construction_plan')return {validationError:null,newCrossingCount:0};
  return loaded;
 });
 const {rerender}=render(<App/>);await screen.findByText(/tram-rescope/);
 fireEvent.click(screen.getByRole('button',{name:'トラム'}));
 const map=document.querySelector('svg[role=img]')!;
 map.getBoundingClientRect=()=>new DOMRect(0,0,1000,600);
 const gesture=(p:typeof start)=>{const xy=project(p.latitude,p.longitude,mapCamera);return {clientX:xy.x,clientY:xy.y,detail:1};};
 if(kind==='undo-road-snap'){
  const [[x1,y1],[x2,y2]]=startTile.tileSnapshots[0].tileLines[0].coordinates;
  const leftPoint=(f:number)=>({latitude:y1+(y2-y1)*f,longitude:x1+(x2-x1)*f});
  const a=leftPoint(.04),b=leftPoint(.48),c=leftPoint(.96);
  nativeReject=false;
  for(const p of [a,b,c])await act(async()=>fireEvent.click(map,gesture(p)));
  const three=document.querySelector('.confirmed-draft')?.getAttribute('d');
  expect(three).toBeTruthy();
  fireEvent.keyDown(window,{key:'z',ctrlKey:true});
  const two=document.querySelector('.confirmed-draft')?.getAttribute('d');
  expect(two).toBeTruthy(); expect(two).not.toBe(three);
  mock.snapshot=endTile;rerender(<App/>);
  // Undo leaves a completed plan; clicking its endpoint resumes that path.
  const follow=screen.getByRole("button",{name:"道路に合わせる"});
  if(follow.getAttribute("aria-pressed")==="false")fireEvent.click(follow);
  await act(async()=>fireEvent.click(map,gesture(b)));
  await act(async()=>fireEvent.click(map,gesture(end)));
  await waitFor(()=>expect(observeWaiting).toBeTruthy());
  expect(mock.observe).toHaveBeenCalledTimes(1);
  await act(async()=>observeWaiting!());
  await waitFor(()=>expect(document.querySelector('.confirmed-draft')?.getAttribute('d')).not.toBe(two));
  const validation=vi.mocked(invoke).mock.calls.filter(([command,args])=>command==='preview_construction_plan'&&(args as {validationOnly?:boolean})?.validationOnly).at(-1)![1] as {alignments:{points:typeof start[]}[]};
  const last=validation.alignments[0].points.at(-1);
  expect(last?.longitude).toBeCloseTo(end.longitude,9);
  expect(last?.latitude).toBeCloseTo(end.latitude,9);
  return;
 }
 await act(async()=>fireEvent.click(map,{...gesture(start),ctrlKey:kind==='free-on-road'}));
 expect(document.querySelector('.draft-endpoint')).toBeTruthy();
 expect(mock.snapshot).toBe(startTile);
 // Publish successor viewport snapshot: the worker is rebuilt with just the
 // end tile; connector still has the old bytes from the first anchor.
 mock.snapshot=endTile;rerender(<App/>);
 await act(async()=>fireEvent.pointerMove(map,gesture(end)));
 expect(mock.observe).not.toHaveBeenCalled();
 expect(mock.read).not.toHaveBeenCalled();
 await act(async()=>fireEvent.click(map,gesture(end)));
 if(kind==='free-on-road'){
  await screen.findByText(/道路の位置を確認できません/);
  expect(mock.observe).not.toHaveBeenCalled();
  expect(mock.read).not.toHaveBeenCalled();
  expect(document.querySelector('.confirmed-draft')).toBeNull();
  return;
 }
 await waitFor(()=>expect(observeWaiting).toBeTruthy());
 expect(mock.observe).toHaveBeenCalledTimes(1);
 const signal=mock.observe.mock.calls[0][2] as AbortSignal;
 expect(mock.observe.mock.calls[0].slice(0,2)).toEqual([[[14551,6452]],revision]);
 fireEvent.click(screen.getByRole('button',{name:'道路に合わせる'}));
 expect(signal.aborted).toBe(true);
 await act(async()=>observeWaiting!());
 expect(mock.read).not.toHaveBeenCalled();
 expect(document.querySelector('.confirmed-draft')).toBeNull();
 fireEvent.click(screen.getByRole('button',{name:'道路に合わせる'}));
 await act(async()=>fireEvent.click(map,gesture(end)));
 await screen.findByText(/試験の勾配拒否/);
 expect(document.querySelector('.confirmed-draft')).toBeNull();
 nativeReject=false;
 await act(async()=>fireEvent.click(map,gesture(end)));
 await waitFor(()=>expect(document.querySelector('.confirmed-draft')).toBeTruthy());
 const native=vi.mocked(invoke).mock.calls.filter(([c,a])=>c==='preview_construction_plan'&&(a as {validationOnly?:boolean})?.validationOnly===true).at(-1)![1] as {alignments:{points:typeof start[];ofmPath?:unknown;trackType:string}[]};
 const placed=native.alignments[0];
 expect(placed.trackType).toBe('tram');
 expect(placed.points.length).toBeGreaterThan(4);
 expect(placed.points[0].latitude).toBeCloseTo(start.latitude,9);
 expect(placed.points[0].longitude).toBeCloseTo(start.longitude,9);
 expect(placed.points.at(-1)?.latitude).toBeCloseTo(end.latitude,9);
 expect(placed.points.at(-1)?.longitude).toBeCloseTo(end.longitude,9);
 expect(placed.ofmPath).toBeUndefined();
 expect(fetcher.mock.calls.map(([url])=>url)).toEqual([OFM_METADATA,tile(14551),tile(14552)]);
 expect(vi.mocked(invoke).mock.calls.some(([c])=>c.startsWith('build_'))).toBe(false);
});

function branchFixture(){
 const p=(offset:number)=>({latitude,longitude:longitude+offset});
 const a=p(-.0008),b=p(.0008);
 const west={id:'10',owner:'2',name:'西側の本線',track_type:'normal',points:[p(-.0015),a]};
 const main={id:'11',owner:'2',name:'既存の中央線',track_type:'normal',points:[a,b]};
 const east={id:'12',owner:'2',name:'東側の本線',track_type:'normal',points:[b,p(.0015)]};
 const end=(alignment_id:string,endpoint:'start'|'end')=>({alignment_id,endpoint});
 const alignment_connections=[{a:end('10','end'),b:end('11','start')},{a:end('11','end'),b:end('12','start')}];
 const snapshot={...world,alignments:[west,main,east],alignment_connections};
 return {snapshot,alignment_connections,main,end};
}
function selectBranch(){
 fireEvent.click(screen.getByRole('button',{name:'分岐として接続'}));
 fireEvent.change(screen.getByLabelText('分岐の起点'),{target:{value:'10:end'}});
 fireEvent.change(screen.getByLabelText('分岐の終点'),{target:{value:'12:start'}});
 fireEvent.click(screen.getByRole('button',{name:'選んだ端点で分岐を計画'}));
}
test('an existing join still blocks ordinary drawing; explicit stem selection uses native preview and build',async()=>{
 const {snapshot,alignment_connections,main,end}=branchFixture();
 const h=await start(snapshot,undefined,false);
 await h.click(-.0008);
 expect(screen.getByText(/この端点は接続済みです/)).toBeTruthy();
 selectBranch();
 expect(screen.getByText(/分岐の終点: 東側の本線/)).toBeTruthy();
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(true);
 const original=vi.mocked(invoke).getMockImplementation()!;
 let deny=true;
 const built={...snapshot,alignments:[...snapshot.alignments,{...main,id:'13',name:'分岐'}],
  alignment_connections:[...alignment_connections,{a:end('10','end'),b:end('13','start')},{a:end('13','end'),b:end('12','start')}]};
 vi.mocked(invoke).mockImplementation(async(command,args)=>{
  if(command==='preview_construction_plan'&&args&&'alignments' in args&&(args.alignments as any[]).some(path=>path.destination))
    return {validationError:deny?'native branch refused':null,newCrossingCount:0};
  if(command==='build_plan')return built;
  return original(command,args);
 });
 await h.click(.0008);
 expect(screen.getByText(/native branch refused/)).toBeTruthy();
 expect(document.querySelector('.confirmed-draft')).toBeNull();
 deny=false;
 await h.click(.00055);
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(true);
 // Roughly 6px from both the selected destination and the last anchor:
 // the explicit terminal has precedence over ordinary finish-on-last-anchor.
 await h.click(.000675);
 expect(document.querySelector('.confirmed-draft')).toBeTruthy();
 fireEvent.click(screen.getByRole('button',{name:'計画を着工'}));
 await waitFor(()=>expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='build_plan')).toBe(true));
 const [,args]=vi.mocked(invoke).mock.calls.find(([c])=>c==='build_plan'&&!!c)!;
 expect(args).toMatchObject({alignments:[{extension:{alignmentId:'10',endpoint:'end'},
   destination:{alignmentId:'12',endpoint:'start'},anchorLevels:['ground','ground','ground'],
   infrastructure:{track_count:1,level:'ground'}}]});
 await waitFor(()=>expect(screen.queryByText(/分岐の終点: 東側の本線/)).toBeNull());
 fireEvent.click(screen.getByRole('button',{name:'分岐として接続'}));
 const pick=screen.getByLabelText('分岐の起点') as HTMLSelectElement;
 expect(Array.from(pick.options).map(option=>option.value)).toEqual(['']);
});

test('clearing an approved branch restores ordinary placement on the same path number and permits deliberate reselection',async()=>{
 const {snapshot}=branchFixture(),h=await start(snapshot,undefined,false);
 selectBranch();
 await h.click(.00055);
 expect(screen.getByText(/分岐の終点: 東側の本線/)).toBeTruthy();
 fireEvent.click(screen.getByRole('button',{name:'下書きを消去'}));
 expect(screen.queryByText(/分岐の終点: 東側の本線/)).toBeNull();
 vi.mocked(invoke).mockClear();
 await h.click(.0025);await h.click(.0035);
 const plans=vi.mocked(invoke).mock.calls.filter(([cmd])=>cmd==='preview_construction_plan')
   .flatMap(([,args])=>args&&'alignments' in args?args.alignments as Array<{extension?:unknown;destination?:unknown;points:unknown[]}>:[]);
 expect(plans.some(plan=>plan.points.length>=2&&!plan.extension&&!plan.destination)).toBe(true);
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
 fireEvent.click(screen.getByRole('button',{name:'下書きを消去'}));
 selectBranch();
 expect(screen.getByText(/分岐の終点: 東側の本線/)).toBeTruthy();
 await h.click(.0008);
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
});

test('local park and return keep a branch permit but a restored draft requires deliberate reselection',async()=>{
 const {snapshot}=branchFixture(),h=await start(snapshot,undefined,false);
 selectBranch();await h.click(.0008);
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
 fireEvent.click(screen.getByRole('button',{name:'別の線路計画を追加'}));
 expect(screen.queryByText(/分岐の終点: 東側の本線/)).toBeNull();
 fireEvent.click(screen.getByRole('button',{name:'計画 1 を編集'}));
 expect(screen.getByText(/分岐の終点: 東側の本線/)).toBeTruthy();
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
 fireEvent.click(screen.getByRole('tab',{name:'セーブデータ'}));
 fireEvent.click(screen.getByRole('button',{name:'ワールドを保存'}));
 await waitFor(()=>expect(vi.mocked(invoke).mock.calls.some(([c])=>c==='save_world')).toBe(true));
 const plans=(vi.mocked(invoke).mock.calls.find(([c])=>c==='save_world')![1] as any).constructionPlans;
 cleanup();await start({...snapshot,construction_plans:JSON.parse(JSON.stringify(plans))},undefined,false);
 expect(screen.queryByText(/分岐の終点: 東側の本線/)).toBeNull();
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(true);
 selectBranch();
 expect((screen.getByRole('button',{name:'計画を着工'}) as HTMLButtonElement).disabled).toBe(false);
});
