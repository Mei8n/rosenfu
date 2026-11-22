import { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { nearestOnAlignment, pointOnAlignment } from './alignment';
export interface RoadTransportData {
 evidence?:unknown[];
 nodes: {id:string;point:{latitude:number;longitude:number}}[];
 edges: {id:string;from:string;to:string;source:string;revision:string;geometry:{latitude:number;longitude:number}[];bus_access:string;tram_access:string;level?:string|null}[];
 bus_stops:{id:string;owner:string;name:string;edge_id:string;position:number;location_kind?:StopLocationKind}[];
 bus_routes:{id:string;owner:string;name:string;edges:string[];stops:{stop_id:string;edge_index:number}[]}[];
 tram_sections:{alignment_id:string;placement:{kind:'dedicated'}|{kind:'shared_road';edges:string[]}}[];
}
export type RoadTransportMutation = 'replace_road_transport' | 'enable_road_qa_fixture' | 'edit_road_tram' | 'install_native_road_pilot' | 'edit_native_pilot_bus';
type StopLocationKind = 'reviewed_reference'|'player_created';
type StopPosition = {edge_id:string;position:number;location_kind?:StopLocationKind};
type RoadEdge = RoadTransportData['edges'][number];
interface NativeRoadRegion {id:string;region:string;origin:string;revision:string;attribution:string;bus_stop_positions:StopPosition[];allows_player_bus_stops?:boolean;}
type PilotBusEdit = {route_id:string;name:string;expected_route:RoadTransportData['bus_routes'][number]|null;from:StopPosition;to:StopPosition};
export type RoadTramEdit = {kind:'build';from:string;to:string;name:string;shared:boolean} | {kind:'change_placement';alignment_id:string;from:string;to:string;shared:boolean} | {kind:'remove';alignment_id:string};
export type RoadMapSelection = {from:string;to:string;revision:number};
export type RoadTransportArgs = {expected:RoadTransportData;replacement:RoadTransportData} | {confirmedSynthetic:true} | {edit:RoadTramEdit|PilotBusEdit} | {assetId:string};
// Small source-geometry inset. Do not show unknown ways as a selectable road.
function RoadPathPicker({data,from,to,pick,disabled}:{data:RoadTransportData;from:string;to:string;pick:(id:string)=>void;disabled:boolean}) {
 const edges=data.edges.filter(e=>e.bus_access==='allowed'||e.tram_access==='allowed');
 if(edges.length===0||edges.length>150)return null;
 const ids=new Set(edges.flatMap(e=>[e.from,e.to]));
 const nodes=data.nodes.filter(n=>ids.has(n.id));
 const points=edges.flatMap(e=>e.geometry);
 if(points.length>2000||nodes.length===0)return null;
 const minLon=Math.min(...points.map(p=>p.longitude)),maxLon=Math.max(...points.map(p=>p.longitude));
 const minLat=Math.min(...points.map(p=>p.latitude)),maxLat=Math.max(...points.map(p=>p.latitude));
 const horizontal=(maxLon-minLon)*Math.cos((maxLat+minLat)/2*Math.PI/180);
 const vertical=maxLat-minLat;
 const scale=140/Math.max(horizontal,vertical,1e-8);
 const x=(longitude:number)=>120+(longitude-(minLon+maxLon)/2)*Math.cos((maxLat+minLat)/2*Math.PI/180)*scale;
 const y=(latitude:number)=>86-((latitude-(minLat+maxLat)/2)*scale);
 return <svg viewBox="0 0 240 172" width="240" height="172" aria-label="道路の起終点を選ぶ略図">
   {edges.map(e=><polyline key={e.id} points={e.geometry.map(p=>`${x(p.longitude)},${y(p.latitude)}`).join(' ')} fill="none" stroke="currentColor" strokeWidth="2" opacity="0.6"/>)}
   {nodes.map(n=><circle key={n.id} role="button" aria-label={`道路点 ${n.id}`} aria-pressed={n.id===from||n.id===to} aria-disabled={disabled} tabIndex={disabled?-1:0} cx={x(n.point.longitude)} cy={y(n.point.latitude)} r="8" fill={n.id===from?'#e8ab44':n.id===to?'#64bb72':'#548cc2'} stroke="#fff" strokeWidth="2"
       onClick={()=>{if(!disabled)pick(n.id);}} onKeyDown={e=>{if(!disabled&&(e.key==='Enter'||e.key===' ')){e.preventDefault();pick(n.id);}}}/>)}
 </svg>;
}
// Choose an explicitly directed edge before setting a position. Overlapping
// reverse edges are not resolved by nearest geometry or silently interchanged.
function FictionalStopPicker({label,edges,value,pick,disabled}:{label:string;edges:RoadEdge[];value?:StopPosition;pick:(value:StopPosition|undefined)=>void;disabled:boolean}) {
 const edge=edges.find(e=>e.id===value?.edge_id);
 function setPosition(position:number){
   if(!disabled&&edge&&position>0&&position<1)pick({edge_id:edge.id,position,location_kind:'player_created'});
 }
 function direction(e:RoadEdge){
   const a=e.geometry[0],b=e.geometry[e.geometry.length-1],dy=b.latitude-a.latitude,dx=(b.longitude-a.longitude)*Math.cos((a.latitude+b.latitude)*Math.PI/360);
   if(dx===0&&dy===0)return '指定方向';
   return Math.abs(dx)>Math.abs(dy)?(dx>0?'東向き':'西向き'):(dy>0?'北向き':'南向き');
 }
 return <div>
   <label>{label}の道路<select value={edge?.id??''} onChange={e=>{
     const id=e.target.value;
     if(!disabled)pick(id?{edge_id:id,position:0.5,location_kind:'player_created'}:undefined);
   }}><option value="">選択</option>{edges.map((e,i)=><option key={e.id} value={e.id}>道路 {i+1}（{direction(e)}）</option>)}</select></label>
   {edge&&value&&<>
     <FictionalStopPreview label={label} edge={edge} position={value.position} pick={setPosition}/>
     <label>{label}の位置<input type="range" min="1" max="99" step="1" value={Math.round(value.position*100)}
       onChange={e=>setPosition(Number(e.target.value)/100)}/></label>
   </>}
 </div>;
}

function FictionalStopPreview({label,edge,position,pick}:{label:string;edge:RoadEdge;position:number;pick:(position:number)=>void}) {
 const points=edge.geometry;
 const [west,east]=[Math.min(...points.map(p=>p.longitude)),Math.max(...points.map(p=>p.longitude))];
 const [south,north]=[Math.min(...points.map(p=>p.latitude)),Math.max(...points.map(p=>p.latitude))];
 const cos=Math.cos((south+north)*Math.PI/360), scale=Math.min(180/Math.max((east-west)*cos,1e-9),74/Math.max(north-south,1e-9));
 const lon=(east+west)/2,lat=(south+north)/2;
 const x=(longitude:number)=>110+(longitude-lon)*cos*scale;
 const y=(latitude:number)=>51-(latitude-lat)*scale;
 const mark=pointOnAlignment(points,position);
 return <svg viewBox="0 0 220 102" width="220" height="102" aria-label={`${label}の道路略図`} onClick={e=>{
   const svg=e.currentTarget, matrix=svg.getScreenCTM?.();
   if(!matrix)return;
   const p=svg.createSVGPoint();p.x=e.clientX;p.y=e.clientY;
   const at=p.matrixTransform(matrix.inverse());
   const nearest=nearestOnAlignment(points,{latitude:lat+(51-at.y)/scale,longitude:lon+(at.x-110)/(cos*scale)});
   if(Math.hypot(x(nearest.point.longitude)-at.x,y(nearest.point.latitude)-at.y)>10)return;
   pick(Math.round(nearest.position*100)/100);
 }}>
   <polyline points={points.map(p=>`${x(p.longitude)},${y(p.latitude)}`).join(' ')} fill="none" stroke="currentColor" strokeWidth="3"/>
   <circle cx={x(points[0].longitude)} cy={y(points[0].latitude)} r="3" fill="currentColor"/>
   <circle cx={x(mark.longitude)} cy={y(mark.latitude)} r="5" fill="#548cc2" stroke="white" strokeWidth="2"/>
 </svg>;
}

export function RoadTransportPanel({data,companyId,run:mutate,enabled=true,mapSelection,onPickOnMap,tramEnabled=false}:{data:RoadTransportData;companyId:string;run:(command:RoadTransportMutation,args:RoadTransportArgs)=>Promise<unknown>;enabled?:boolean;mapSelection?:RoadMapSelection;onPickOnMap?:()=>void;tramEnabled?:boolean}) {
 const [confirmed,setConfirmed]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState('');
 const [from,setFrom]=useState(''),[to,setTo]=useState(''),[name,setName]=useState('バス系統');
 const [tramName,setTramName]=useState('軌道'),[shared,setShared]=useState(true);
 const [regions,setRegions]=useState<NativeRoadRegion[]>();
 const [regionsFailed,setRegionsFailed]=useState(false);
 const [pilotFrom,setPilotFrom]=useState(''),[pilotTo,setPilotTo]=useState('');
 const [playerFrom,setPlayerFrom]=useState<StopPosition>(),[playerTo,setPlayerTo]=useState<StopPosition>();
 const [editing,setEditing]=useState<RoadTransportData['bus_routes'][number]>();
 useEffect(()=>{
   let current=true;
   void (async()=>{
     try {
       const value=await invoke<NativeRoadRegion[]>('list_native_road_regions');
       if(!Array.isArray(value))throw Error('Invalid region list');
       if(current)setRegions(value);
     } catch {
       if(current)setRegionsFailed(true);
     }
   })();
   return ()=>{current=false;};
 },[]);
 const qa=data.edges.length>0&&data.edges.every(edge=>edge.source==='synthetic-qa-not-real-roads');
 const pilot=regions?.find(region=>data.edges.length>0&&data.edges.every(edge=>edge.source===`native-pilot:${region.id}`&&edge.revision===region.revision));
 const hasGraph=data.edges.length>0||data.nodes.length>0||Boolean(data.evidence?.length);
 const stopKey=(point:StopPosition)=>JSON.stringify([point.edge_id,point.position]);
 const boardingChoices=pilot?.bus_stop_positions??[];
 const allowsPlayer=pilot?.allows_player_bus_stops===true;
 const playerEdges=useMemo(()=>allowsPlayer?data.edges.filter(edge=>edge.bus_access==='allowed'&&edge.level==='ground'&&edge.geometry.length>=2):[],[allowsPlayer,data.edges]);
 const playerChoice=(point:StopPosition|undefined)=>point&&point.position>0&&point.position<1&&playerEdges.some(e=>e.id===point.edge_id)?point:undefined;
 const fromPosition=playerChoice(playerFrom)??boardingChoices.find(point=>stopKey(point)===pilotFrom);
 const toPosition=playerChoice(playerTo)??boardingChoices.find(point=>stopKey(point)===pilotTo);
 const pilotEndpointsValid=!!fromPosition&&!!toPosition&&(fromPosition.edge_id!==toPosition.edge_id||fromPosition.position<toPosition.position);
 const ownRoutes=data.bus_routes.filter(route=>route.owner===companyId);
 useEffect(()=>{if(mapSelection){setFrom(mapSelection.from);setTo(mapSelection.to);}},[mapSelection]);
 function pickInset(id:string){if(!from||to){setFrom(id);setTo('');}else if(id!==from){setTo(id);}}
 const endpointValid=from!==to && data.nodes.some(n=>n.id===from) && data.nodes.some(n=>n.id===to);
 const editTram=(edit:RoadTramEdit)=>mutate('edit_road_tram',{edit});
 async function run(job:()=>Promise<unknown>){if(busy||!enabled)return;setBusy(true);setError('');try{await job();}catch(e){setError(String(e));}finally{setBusy(false);}}
 async function save(replacement:RoadTransportData){return mutate('replace_road_transport',{expected:data,replacement});}
 async function add(){const edges=await invoke<string[]>('find_road_path',{from,to,tram:false});if(!edges.length)throw new Error('経路がありません');const id=`bus:${crypto.randomUUID()}`;const stops=[{id:`${id}:start`,owner:companyId,name:'起点',edge_id:edges[0],position:0},{id:`${id}:end`,owner:companyId,name:'終点',edge_id:edges.at(-1)!,position:1}];return save({...data,bus_stops:[...data.bus_stops,...stops],bus_routes:[...data.bus_routes,{id,owner:companyId,name:name.trim(),edges,stops:stops.map((s,i)=>({stop_id:s.id,edge_index:i?edges.length-1:0}))}]});}
 function editPilot(route:RoadTransportData['bus_routes'][number]){
   const first=data.bus_stops.find(s=>s.id===route.stops[0]?.stop_id);
   const last=data.bus_stops.find(s=>s.id===route.stops.at(-1)?.stop_id);
   setEditing(route);setName(route.name);
   const restore=(stop:StopPosition|undefined,setPlayer:(value:StopPosition|undefined)=>void,setReference:(value:string)=>void)=>{
     const custom=stop?.location_kind==='player_created';
     setPlayer(custom&&stop?{edge_id:stop.edge_id,position:stop.position,location_kind:'player_created'}:undefined);
     setReference(stop&&!custom?stopKey(stop):'');
   };
   restore(first,setPlayerFrom,setPilotFrom);
   restore(last,setPlayerTo,setPilotTo);
 }
 function clearPilot(){
   setEditing(undefined);setPilotFrom('');setPilotTo('');setPlayerFrom(undefined);setPlayerTo(undefined);setName('バス系統');
 }
 async function savePilot(){
   if(!fromPosition||!toPosition||!pilotEndpointsValid)return;
   await mutate('edit_native_pilot_bus',{edit:{route_id:editing?.id??`bus:${crypto.randomUUID()}`,
     name:name.trim(),expected_route:editing??null,from:fromPosition,to:toPosition}});
   setEditing(undefined);
 }
 return <section aria-label="道路交通"><h3>道路交通</h3>
 <section aria-label="確認済みの実在道路">
  <h4>確認済みの実在道路</h4>
  {regionsFailed?<p role="alert">道路データの一覧を確認できませんでした。</p>:
   !regions?<p role="status">道路データを確認中</p>:
   regions.length===0?<p role="status">この版で追加できる承認済みの実在地域はありません。</p>:
   regions.map(region=><div key={region.id}>
     <strong>{region.region}</strong><p>出所: {region.origin} / 版: {region.revision} / 帰属: {region.attribution}</p>
     {pilot?.id===region.id?<span>このワールドで使用中</span>:<button type="button" disabled={hasGraph||!enabled||busy}
       onClick={()=>void run(()=>mutate('install_native_road_pilot',{assetId:region.id}))}>{region.region}を追加</button>}
   </div>)}
  {hasGraph&&!pilot&&Boolean(regions?.length)&&<p>このワールドには道路データがあるため、上書きできません。</p>}
 </section>
 {!hasGraph?<><label><input type="checkbox" checked={confirmed} onChange={e=>setConfirmed(e.target.checked)}/>実在道路ではない検証用データを使用する</label><button disabled={!enabled||!confirmed||busy} onClick={()=>void run(()=>mutate('enable_road_qa_fixture',{confirmedSynthetic:true}))}>検証用道路を追加</button></>:<p role="status">{qa?'検証専用の架空道路。実在の地図・通行可能範囲ではありません。':pilot?(allowsPlayer?'確認済み道路の通行方向を使います。ゲーム内停留所は実在の停留所ではありません。':'承認された方向・経路・停留所だけを使用します。'):'この道路データは参照のみで、経路を作成できません。'}</p>}
 {(qa||pilot)&&<>
 {!onPickOnMap&&<RoadPathPicker data={data} from={from} to={to} disabled={!enabled||busy} pick={pickInset}/>}
 {onPickOnMap&&<button type="button" disabled={!enabled||busy} onClick={onPickOnMap}>地図で起点・終点を選ぶ</button>}
 <label>起点<select value={from} onChange={e=>setFrom(e.target.value)}><option value="">選択</option>{data.nodes.map(n=><option key={n.id}>{n.id}</option>)}</select></label>
 <label>終点<select value={to} onChange={e=>setTo(e.target.value)}><option value="">選択</option>{data.nodes.map(n=><option key={n.id}>{n.id}</option>)}</select></label>
 {pilot&&boardingChoices.length>0&&<fieldset disabled={!enabled||busy}><legend>承認済みバス停位置</legend>
   <label>乗車<select value={pilotFrom} onChange={e=>{setPilotFrom(e.target.value);setPlayerFrom(undefined);}}><option value="">選択</option>{boardingChoices.map(point=><option key={stopKey(point)} value={stopKey(point)}>{point.edge_id} / {point.position*100}%</option>)}</select></label>
   <label>降車<select value={pilotTo} onChange={e=>{setPilotTo(e.target.value);setPlayerTo(undefined);}}><option value="">選択</option>{boardingChoices.map(point=><option key={stopKey(point)} value={stopKey(point)}>{point.edge_id} / {point.position*100}%</option>)}</select></label>
 </fieldset>}
 {allowsPlayer&&playerEdges.length>0&&<fieldset disabled={!enabled||busy}><legend>ゲーム内停留所を置く</legend>
   <p>道路の向きを選んで、略図かスライダーで位置を決めます。</p>
   <FictionalStopPicker label="乗車" edges={playerEdges} disabled={!enabled||busy} value={playerFrom} pick={point=>{setPlayerFrom(point);if(point)setPilotFrom('');}}/>
   <FictionalStopPicker label="降車" edges={playerEdges} disabled={!enabled||busy} value={playerTo} pick={point=>{setPlayerTo(point);if(point)setPilotTo('');}}/>
 </fieldset>}
 <label>名称<input autoComplete="off" value={name} onChange={e=>setName(e.target.value)}/></label>
 {pilot?<><button disabled={!enabled||busy||!pilotEndpointsValid||!name.trim()} onClick={()=>void run(savePilot)}>{editing?'バス系統の変更を適用':allowsPlayer?'停留所とバス系統を作成':'承認済み停留所でバス系統を作成'}</button>{editing&&<button disabled={!enabled||busy} onClick={clearPilot}>編集をやめる</button>}</>:<button disabled={!enabled||busy||!endpointValid||!name.trim()} onClick={()=>void run(add)}>停留所とバス系統を作成</button>}
 {ownRoutes.map(route=><div key={route.id}><span>{route.name}（{route.stops.length}停留所）</span>
   {pilot&&route.stops.length===2&&<button disabled={!enabled||busy} onClick={()=>editPilot(route)}>系統を編集</button>}
   <button disabled={!enabled||busy} onClick={()=>void run(async()=>{await save({...data,bus_routes:data.bus_routes.filter(r=>r.id!==route.id)});if(editing?.id===route.id)setEditing(undefined);})}>系統を削除</button></div>)}
 {pilot&&data.bus_stops.filter(s=>s.owner===companyId&&!data.bus_routes.some(route=>route.stops.some(o=>o.stop_id===s.id))).map(s=><button key={s.id} disabled={!enabled||busy} onClick={()=>void run(()=>save({...data,bus_stops:data.bus_stops.filter(stop=>stop.id!==s.id)}))}>未使用停留所 {s.id} を削除</button>)}
 <p>系統削除時は停留所を残します。現在の道路操作はワールドのUndo/Redo・保存の対象です。</p>
 {tramEnabled&&<fieldset disabled={!enabled||busy}><legend>軌道</legend>
   <label>軌道名<input autoComplete="off" value={tramName} onChange={e=>setTramName(e.target.value)}/></label>
   <label><input type="checkbox" checked={shared} onChange={e=>setShared(e.target.checked)}/>道路と関連付ける（併用軌道）</label>
   <p>選択した道路の形を使用します。関連なしの場合も軌道の位置はずらしません。道路運行や踏切扱いは未対応です。</p>
   <button type="button" disabled={!endpointValid||!tramName.trim()} onClick={()=>void run(()=>editTram({kind:'build',from,to,name:tramName.trim(),shared}))}>軌道を着工</button>
   {data.tram_sections.map(t=><div key={t.alignment_id}>
    <span>{t.placement.kind==='shared_road'?'併用軌道':'道路関連なし'} #{t.alignment_id}</span>
    <button type="button" disabled={t.placement.kind==='dedicated'&&!endpointValid} onClick={()=>void run(()=>editTram({kind:'change_placement',alignment_id:t.alignment_id,from,to,shared:t.placement.kind==='dedicated'}))}>{t.placement.kind==='dedicated'?'選択道路を関連付ける':'道路との関連を外す'}</button>
    <button type="button" onClick={()=>{if(window.confirm('この軌道を撤去しますか？'))void run(()=>editTram({kind:'remove',alignment_id:t.alignment_id}));}}>軌道を撤去</button>
   </div>)}
 </fieldset>}</>}{error&&<p role="alert">{error}</p>}</section>;
}
