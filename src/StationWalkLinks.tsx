import {useMemo,useState,type FormEvent} from 'react';

type Group={station_id:string;members:string[]};
type Station={id:string;owner:string;name:string;latitude:number;longitude:number};
export type StationWalkLink={from:string;to:string;seconds:number};
export type StationWalkEdit={from:string;to:string;seconds:number|null;bothDirections:boolean};
const radians=(n:number)=>n*Math.PI/180;
const metres=(a:Station,b:Station)=>6371000*Math.hypot(radians(b.latitude-a.latitude),
 radians(b.longitude-a.longitude)*Math.cos(radians((a.latitude+b.latitude)/2)));

export function StationWalkLinks({station,stations,groups,links,enabled,onEdit}:{
 station:Station;stations:Station[];groups?:Group[];links:StationWalkLink[];enabled:boolean;onEdit:(edit:StationWalkEdit)=>void;
}){
 const [partner,setPartner]=useState(''),[minutes,setMinutes]=useState(''),[both,setBoth]=useState(true);
 const candidates=useMemo(()=>{
  const sameStation=new Set(groups?.find(g=>g.members.includes(station.id))?.members??[station.id]);
  return stations.filter(s=>!sameStation.has(s.id)&&s.owner===station.owner).map(s=>({s,distance:metres(station,s)}))
   .filter(({distance})=>distance<=1000).sort((a,b)=>a.distance-b.distance);
 },[station,stations,groups]);
 const chosen=candidates.find(({s})=>s.id===partner);
 const duplicate=links.some(link=>link.from===station.id&&link.to===partner||both&&link.from===partner&&link.to===station.id);
 const seconds=Number(minutes)*60;
 const valid=enabled&&!!chosen&&!duplicate&&/^\d{1,2}$/.test(minutes)&&seconds>=60&&seconds<=3600;
 const local=links.filter(link=>link.from===station.id||link.to===station.id);
 const name=(id:string)=>stations.find(s=>s.id===id)?.name??id;
 function add(event:FormEvent<HTMLFormElement>){event.preventDefault();if(valid)onEdit({from:station.id,to:partner,seconds,bothDirections:both});}
 return <details>
  <summary>駅間の徒歩連絡</summary>
  <p>自社の別々の駅を直線1 km以内で結びます。駅名・駅IDは変わりません。選んで登録した連絡だけがダイヤを使った需要試算に反映されます。</p>
  {candidates.length>0?<form noValidate onSubmit={add}><fieldset disabled={!enabled}>
   <label>連絡先の駅<select value={partner} onChange={event=>setPartner(event.target.value)}>
    <option value="">駅を選択</option>{candidates.map(({s,distance})=><option key={s.id} value={s.id}>{s.name}（直線 約{Math.round(distance)} m）</option>)}
   </select></label>
   <label>徒歩連絡の時間（分）<input type="number" min="1" max="60" step="1" value={minutes} onChange={event=>setMinutes(event.target.value)}/></label>
   <label><input type="checkbox" checked={both} onChange={event=>setBoth(event.target.checked)}/>両方向に登録</label>
   {!both&&chosen&&<p>片方向: {station.name} から {chosen.s.name}</p>}
   {duplicate&&<p>指定した方向に徒歩連絡が登録済みです。</p>}
   <button type="submit" disabled={!valid}>徒歩連絡を登録</button>
  </fieldset></form>:<p>近くに連絡先の候補がありません。</p>}
  {local.length>0&&<ul aria-label="登録した徒歩連絡">{local.map(link=><li key={`${link.from}:${link.to}`}>
   {name(link.from)} → {name(link.to)}（{link.seconds/60}分）
   <button type="button" className="secondary" disabled={!enabled}
    aria-label={`${name(link.from)} から ${name(link.to)} の徒歩連絡を外す`}
    onClick={()=>onEdit({from:link.from,to:link.to,seconds:null,bothDirections:false})}>この方向を外す</button>
  </li>)}</ul>}
 </details>;
}
