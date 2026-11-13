import {useEffect,useMemo,useRef,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import type {Segment} from "./RouteCandidatePreview";
type Route={id:string;name:string;start:string;tracks:string[]};type Track={id:string;from:string;to:string};type RoutePath={route_id:string;segments:Segment[]};
export type ServiceRoutePreview={route_id:string;start:string;tracks:string[];stations:{station_id:string;occurrence:number;route_occurrence:number;route_member:boolean}[];requires_stop_selection:boolean};
export type ServiceImportDraft={start:string;tracks:string[];stops:number[]};
export function trackRouteOccurrences(route:Route,tracks:Track[],path?:RoutePath):string[]|null{
 if(path?.segments.some(segment=>segment.kind!=="track"))return null;
 const ids=path?path.segments.flatMap(segment=>segment.kind==="track"?[segment.track_id]:[]):route.tracks;
 const byId=new Map(tracks.map(track=>[track.id,track]));const stations=[route.start];
 for(const id of ids){const edge=byId.get(id),last=stations.at(-1)!;if(!edge || (edge.from!==last&&edge.to!==last))return null;stations.push(edge.from===last?edge.to:edge.from);}
 return stations;
}
export function ServiceFromRoutePreview({routes,tracks,routePaths,stations,worldVersion,enabled,canApply,onApply}:{routes:Route[];tracks:Track[];routePaths:RoutePath[];stations:{id:string;name:string}[];worldVersion:unknown;enabled:boolean;canApply:boolean;onApply:(draft:ServiceImportDraft)=>void}){
 const [routeId,setRouteId]=useState(""),[from,setFrom]=useState("0"),[to,setTo]=useState("0");
 const [preview,setPreview]=useState<ServiceRoutePreview|null>(null),[policy,setPolicy]=useState<"all"|"custom"|null>(null),[stops,setStops]=useState<number[]>([]),[confirmed,setConfirmed]=useState(false),[loading,setLoading]=useState(false),[error,setError]=useState("");
 const generation=useRef(0),pending=useRef(false);const names=new Map(stations.map(station=>[station.id,station.name]));
 const choices=useMemo(()=>routes.map(route=>({route,occurrences:trackRouteOccurrences(route,tracks,routePaths.find(path=>path.route_id===route.id))})),[routes,tracks,routePaths]);
 const chosen=choices.find(choice=>choice.route.id===routeId),occurrences=chosen?.occurrences??[];
 function clear(){generation.current++;pending.current=false;setPreview(null);setPolicy(null);setStops([]);setConfirmed(false);setLoading(false);setError("");}
 useEffect(()=>{clear();return ()=>{generation.current++;};},[worldVersion]);
 async function inspect(){
  if(!enabled||pending.current||!chosen||!occurrences.length||from===to||![Number(from),Number(to)].every(index=>Number.isInteger(index)&&index>=0&&index<occurrences.length))return;
  clear();const token=generation.current;pending.current=true;setLoading(true);
  try{const response=await invoke<ServiceRoutePreview>("preview_service_from_route",{routeId,fromOccurrence:Number(from),toOccurrence:Number(to)});if(token!==generation.current)return;
   const step=Number(to)>Number(from)?1:-1;
   const path=routePaths.find(path=>path.route_id===routeId);
   const sourceTracks=path?path.segments.flatMap(segment=>segment.kind==="track"?[segment.track_id]:[]):chosen.route.tracks;
   const expectedTracks=sourceTracks.slice(Math.min(Number(from),Number(to)),Math.max(Number(from),Number(to)));if(step<0)expectedTracks.reverse();
   if(response.route_id!==routeId||response.requires_stop_selection!==true||!Array.isArray(response.tracks)||!Array.isArray(response.stations)||response.tracks.length!==expectedTracks.length||response.tracks.some((track,index)=>track!==expectedTracks[index])||!response.tracks.length||response.stations.length!==response.tracks.length+1||response.stations.length!==Math.abs(Number(to)-Number(from))+1||response.start!==response.stations[0]?.station_id||response.stations.some((station,index)=>station.occurrence!==index||station.route_occurrence!==Number(from)+index*step||station.station_id!==occurrences[station.route_occurrence]||typeof station.route_member!=="boolean"))throw Error("経路プレビューの形式が一致しません。再確認してください。");
   setPreview(response);
  }catch(cause){if(token===generation.current)setError(String(cause));}finally{if(token===generation.current){pending.current=false;setLoading(false);}}
 }
 return <section className="service-route-preview" aria-label="路線区間から新規系統下書き"><h4>路線区間から新規系統下書き</h4><p className="field-hint">駅間接続（Track）のみの路線が対象。建設線形を含む路線は部分区間もこのUIでは未対応です。路線所属と停車/通過は別に確認します。</p>
 <fieldset disabled={!enabled}><label htmlFor="service-source-route">元にする路線</label><select id="service-source-route" value={routeId} onChange={event=>{clear();setRouteId(event.target.value);setFrom("0");const target=choices.find(choice=>choice.route.id===event.target.value);setTo(String(Math.max(0,(target?.occurrences?.length??1)-1)));}}><option value="">路線を選択</option>{choices.map(({route,occurrences})=><option key={route.id} value={route.id} disabled={!occurrences||occurrences.length<2}>{route.name}{!occurrences?"（建設線形または未解決経路・未対応）":occurrences.length<2?"（区間なし）":""}</option>)}</select>
 <label htmlFor="service-source-from">区間の出発位置</label><select id="service-source-from" value={from} onChange={event=>{clear();setFrom(event.target.value);}}>{occurrences.map((id,index)=><option key={index} value={index}>{index+1}: {names.get(id)??id}</option>)}</select>
 <label htmlFor="service-source-to">区間の終点位置</label><select id="service-source-to" value={to} onChange={event=>{clear();setTo(event.target.value);}}>{occurrences.map((id,index)=><option key={index} value={index}>{index+1}: {names.get(id)??id}</option>)}</select>
 <p className="field-hint">番号は路線上の出現順。再訪する同名駅も位置別、逆方向も選択できます。</p><button type="button" disabled={!enabled||loading||occurrences.length<2||from===to} onClick={()=>void inspect()}>系統用の区間を確認</button></fieldset>
 {loading&&<p role="status">系統用の経路を確認中…</p>}{error&&<p role="alert">{error}</p>}
 {preview&&<><fieldset disabled={!enabled||loading}><legend>停車方針（選択必須）</legend><label><input type="radio" name="service-stop-policy" checked={policy==="all"} onChange={()=>{setPolicy("all");setStops(preview.stations.map(station=>station.occurrence));setConfirmed(false);}}/>全駅停車（路線所属外も含む）</label><label><input type="radio" name="service-stop-policy" checked={policy==="custom"} onChange={()=>{setPolicy("custom");setStops(preview.stations.map(station=>station.occurrence));setConfirmed(false);}}/>各駅を個別指定（初期表示は全駅停車）</label>
 <ol>{preview.stations.map((station,index)=><li key={index}><label><input type="checkbox" aria-label={`候補停車 ${index+1}: ${names.get(station.station_id)??station.station_id}`} checked={policy!==null&&stops.includes(index)} disabled={policy!=="custom"||index===0||index===preview.stations.length-1} onChange={event=>{setStops(previous=>event.target.checked?[...previous,index].sort((a,b)=>a-b):previous.filter(stop=>stop!==index));setConfirmed(false);}}/>{index+1}. {names.get(station.station_id)??station.station_id} / 元路線{station.route_occurrence+1}番目 / {station.route_member?"路線所属":"路線所属外"} / {policy===null?"停車方針未選択":stops.includes(index)?"停車":"通過"}{index===0||index===preview.stations.length-1?"（端点・停車固定）":""}</label></li>)}</ol>
 <label><input type="checkbox" checked={confirmed} disabled={policy===null} onChange={event=>setConfirmed(event.target.checked)}/>所属外駅を含め、表示の停車・通過を確認しました</label></fieldset>
 <button type="button" disabled={!enabled||!canApply||policy===null||!confirmed||loading} onClick={()=>{if(!enabled||!canApply||pending.current||!policy||!confirmed)return;onApply({start:preview.start,tracks:[...preview.tracks],stops:[...stops]});clear();}}>新規系統の下書きへ取り込む</button>
 {!canApply&&<p className="field-hint">既存系統を編集中、または未適用の系統下書きがあります。先に適用/破棄して「新規系統」へ切り替えてください。</p>}<p className="field-hint">取込は下書きのみ。独立した系統名を入力し、停車設定を確認して「系統を適用」してください。</p></>}
 </section>;
}
