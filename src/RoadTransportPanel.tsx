import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
export interface RoadTransportData {
 evidence?:unknown[];
 nodes: {id:string;point:{latitude:number;longitude:number}}[];
 edges: {id:string;from:string;to:string;source:string;revision:string;geometry:{latitude:number;longitude:number}[];bus_access:string;tram_access:string}[];
 bus_stops:{id:string;owner:string;name:string;edge_id:string;position:number}[];
 bus_routes:{id:string;owner:string;name:string;edges:string[];stops:{stop_id:string;edge_index:number}[]}[];
 tram_sections:unknown[];
}
export type RoadTransportMutation = 'replace_road_transport' | 'enable_road_qa_fixture';
export type RoadTransportArgs = {expected:RoadTransportData;replacement:RoadTransportData} | {confirmedSynthetic:true};
export function RoadTransportPanel({data,companyId,run:mutate,enabled=true}:{data:RoadTransportData;companyId:string;run:(command:RoadTransportMutation,args:RoadTransportArgs)=>Promise<unknown>;enabled?:boolean}) {
 const [confirmed,setConfirmed]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState('');
 const [from,setFrom]=useState(''),[to,setTo]=useState(''),[name,setName]=useState('バス系統');
 async function run(job:()=>Promise<unknown>){if(busy||!enabled)return;setBusy(true);setError('');try{await job();}catch(e){setError(String(e));}finally{setBusy(false);}}
 async function save(replacement:RoadTransportData){return mutate('replace_road_transport',{expected:data,replacement});}
 async function add(){const edges=await invoke<string[]>('find_road_path',{from,to,tram:false});if(!edges.length)throw new Error('経路がありません');const id=`bus:${crypto.randomUUID()}`;const stops=[{id:`${id}:start`,owner:companyId,name:'起点',edge_id:edges[0],position:0},{id:`${id}:end`,owner:companyId,name:'終点',edge_id:edges.at(-1)!,position:1}];return save({...data,bus_stops:[...data.bus_stops,...stops],bus_routes:[...data.bus_routes,{id,owner:companyId,name:name.trim(),edges,stops:stops.map((s,i)=>({stop_id:s.id,edge_index:i?edges.length-1:0}))}]});}
 return <section aria-label="道路交通"><h3>道路交通</h3><p>実在道路の通行条件と接続は未検証です。実在道路での運行はまだ利用できません。</p>
 {!data.edges.length?<><label><input type="checkbox" checked={confirmed} onChange={e=>setConfirmed(e.target.checked)}/>実在道路ではない検証用データを使用する</label><button disabled={!enabled||!confirmed||busy} onClick={()=>void run(()=>mutate('enable_road_qa_fixture',{confirmedSynthetic:true}))}>検証用道路を追加</button></>:<><p role="status">{data.edges.every(e=>e.source==='synthetic-qa-not-real-roads')?'検証専用の架空道路。実在の地図・通行可能範囲ではありません。':'道路データ。通行不明区間は経路から除外します。'}</p>
 <label>起点<select value={from} onChange={e=>setFrom(e.target.value)}><option value="">選択</option>{data.nodes.map(n=><option key={n.id}>{n.id}</option>)}</select></label>
 <label>終点<select value={to} onChange={e=>setTo(e.target.value)}><option value="">選択</option>{data.nodes.map(n=><option key={n.id}>{n.id}</option>)}</select></label>
 <label>名称<input autoComplete="off" value={name} onChange={e=>setName(e.target.value)}/></label><button disabled={!enabled||busy||!from||!to||from===to||!name.trim()} onClick={()=>void run(add)}>停留所とバス系統を作成</button>
 {data.bus_routes.filter(r=>r.owner===companyId).map(route=><div key={route.id}><span>{route.name}（{route.stops.length}停留所）</span><button disabled={!enabled||busy} onClick={()=>void run(()=>save({...data,bus_routes:data.bus_routes.filter(r=>r.id!==route.id)}))}>系統を削除</button></div>)}
 <p>系統削除時は停留所を残します。現在の道路操作はワールドのUndo/Redo・保存の対象です。</p></>}{error&&<p role="alert">{error}</p>}</section>;
}
