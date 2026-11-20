import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
export interface RoadTransportData {
 evidence?:unknown[];
 nodes: {id:string;point:{latitude:number;longitude:number}}[];
 edges: {id:string;from:string;to:string;source:string;revision:string;geometry:{latitude:number;longitude:number}[];bus_access:string;tram_access:string}[];
 bus_stops:{id:string;owner:string;name:string;edge_id:string;position:number}[];
 bus_routes:{id:string;owner:string;name:string;edges:string[];stops:{stop_id:string;edge_index:number}[]}[];
 tram_sections:{alignment_id:string;placement:{kind:'dedicated'}|{kind:'shared_road';edges:string[]}}[];
}
export type RoadTransportMutation = 'replace_road_transport' | 'enable_road_qa_fixture' | 'edit_road_tram';
export type RoadTramEdit = {kind:'build';from:string;to:string;name:string;shared:boolean} | {kind:'change_placement';alignment_id:string;from:string;to:string;shared:boolean} | {kind:'remove';alignment_id:string};
export type RoadMapSelection = {from:string;to:string;revision:number};
export type RoadTransportArgs = {expected:RoadTransportData;replacement:RoadTransportData} | {confirmedSynthetic:true} | {edit:RoadTramEdit};
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
export function RoadTransportPanel({data,companyId,run:mutate,enabled=true,mapSelection,onPickOnMap,tramEnabled=false}:{data:RoadTransportData;companyId:string;run:(command:RoadTransportMutation,args:RoadTransportArgs)=>Promise<unknown>;enabled?:boolean;mapSelection?:RoadMapSelection;onPickOnMap?:()=>void;tramEnabled?:boolean}) {
 const [confirmed,setConfirmed]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState('');
 const [from,setFrom]=useState(''),[to,setTo]=useState(''),[name,setName]=useState('バス系統');
 const [tramName,setTramName]=useState('軌道'),[shared,setShared]=useState(true);
 useEffect(()=>{if(mapSelection){setFrom(mapSelection.from);setTo(mapSelection.to);}},[mapSelection]);
 function pickInset(id:string){if(!from||to){setFrom(id);setTo('');}else if(id!==from){setTo(id);}}
 const endpointValid=from!==to && data.nodes.some(n=>n.id===from) && data.nodes.some(n=>n.id===to);
 const editTram=(edit:RoadTramEdit)=>mutate('edit_road_tram',{edit});
 async function run(job:()=>Promise<unknown>){if(busy||!enabled)return;setBusy(true);setError('');try{await job();}catch(e){setError(String(e));}finally{setBusy(false);}}
 async function save(replacement:RoadTransportData){return mutate('replace_road_transport',{expected:data,replacement});}
 async function add(){const edges=await invoke<string[]>('find_road_path',{from,to,tram:false});if(!edges.length)throw new Error('経路がありません');const id=`bus:${crypto.randomUUID()}`;const stops=[{id:`${id}:start`,owner:companyId,name:'起点',edge_id:edges[0],position:0},{id:`${id}:end`,owner:companyId,name:'終点',edge_id:edges.at(-1)!,position:1}];return save({...data,bus_stops:[...data.bus_stops,...stops],bus_routes:[...data.bus_routes,{id,owner:companyId,name:name.trim(),edges,stops:stops.map((s,i)=>({stop_id:s.id,edge_index:i?edges.length-1:0}))}]});}
 return <section aria-label="道路交通"><h3>道路交通</h3><p>実在道路の通行条件と接続は未検証です。実在道路での運行はまだ利用できません。</p>
 {!data.edges.length?<><label><input type="checkbox" checked={confirmed} onChange={e=>setConfirmed(e.target.checked)}/>実在道路ではない検証用データを使用する</label><button disabled={!enabled||!confirmed||busy} onClick={()=>void run(()=>mutate('enable_road_qa_fixture',{confirmedSynthetic:true}))}>検証用道路を追加</button></>:<><p role="status">{data.edges.every(e=>e.source==='synthetic-qa-not-real-roads')?'検証専用の架空道路。実在の地図・通行可能範囲ではありません。':'道路データ。通行不明区間は経路から除外します。'}</p>
 {!onPickOnMap&&<RoadPathPicker data={data} from={from} to={to} disabled={!enabled||busy} pick={pickInset}/>}
 {onPickOnMap&&<button type="button" disabled={!enabled||busy} onClick={onPickOnMap}>地図で起点・終点を選ぶ</button>}
 <label>起点<select value={from} onChange={e=>setFrom(e.target.value)}><option value="">選択</option>{data.nodes.map(n=><option key={n.id}>{n.id}</option>)}</select></label>
 <label>終点<select value={to} onChange={e=>setTo(e.target.value)}><option value="">選択</option>{data.nodes.map(n=><option key={n.id}>{n.id}</option>)}</select></label>
 <label>名称<input autoComplete="off" value={name} onChange={e=>setName(e.target.value)}/></label><button disabled={!enabled||busy||!from||!to||from===to||!name.trim()} onClick={()=>void run(add)}>停留所とバス系統を作成</button>
 {data.bus_routes.filter(r=>r.owner===companyId).map(route=><div key={route.id}><span>{route.name}（{route.stops.length}停留所）</span><button disabled={!enabled||busy} onClick={()=>void run(()=>save({...data,bus_routes:data.bus_routes.filter(r=>r.id!==route.id)}))}>系統を削除</button></div>)}
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
