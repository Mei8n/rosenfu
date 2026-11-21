import {useRef,useState,type FormEvent} from 'react';
import {invoke} from '@tauri-apps/api/core';

type Named={id:string;name:string};
type Ride={service_id:string;from:string;to:string;departed_ms:number;arrived_ms:number};
type Replay={
 version:1;ready_ms:number;observed_until_ms:number;requested_until_ms:number;
 status:'reached'|'no_arrival_within_window'|'incomplete';arrival_ms:number|null;
 platform_wait_ms:number;onboard_ms:number;rides:Ride[];
};
const clock=(ms:number)=>{
 const seconds=Math.floor(ms/1_000);
 return [Math.floor(seconds/3_600),Math.floor(seconds/60)%60,seconds%60].map(n=>String(n).padStart(2,'0')).join(':');
};

export function RailJourneyReplayPanel({stations,services,enabled,worldVersion}:{stations:Named[];services:Named[];enabled:boolean;worldVersion:unknown}){
 const [origin,setOrigin]=useState(''),[destination,setDestination]=useState('');
 const [pending,setPending]=useState(false),[settled,setSettled]=useState<{version:unknown;value:Replay|string}>();
 const latest=useRef({worldVersion,enabled}),running=useRef(false);
 latest.current={worldVersion,enabled};
 const names=new Map(stations.map(x=>[x.id,x.name]));
 const serviceNames=new Map(services.map(x=>[x.id,x.name]));
 const valid=origin!==destination&&names.has(origin)&&names.has(destination);
 const value=settled?.version===worldVersion?settled?.value:undefined;
 async function calculate(event:FormEvent){
  event.preventDefault();
  if(!enabled||running.current||!valid)return;
  running.current=true;setPending(true);setSettled(undefined);
  const version=worldVersion;
  try {
   const result=await invoke<Replay>('preview_rail_journey_replay',{origin,destination});
   if(!result||result.version!==1||!['reached','no_arrival_within_window','incomplete'].includes(result.status))throw Error('乗車案内の応答を確認できませんでした。');
   if(latest.current.worldVersion===version&&latest.current.enabled)setSettled({version,value:result});
  }catch(cause){
   if(latest.current.worldVersion===version&&latest.current.enabled)setSettled({version,value:String(cause)});
  }finally{running.current=false;setPending(false);}
 }
 const select=(label:string,value:string,set:(id:string)=>void)=><label>{label}
  <select required value={value} onChange={event=>{set(event.target.value);setSettled(undefined);}}>
   <option value="">駅を選択</option>
   {stations.map(station=><option key={station.id} value={station.id}>{station.name}</option>)}
  </select>
 </label>;
 return <section className="fixed-od-panel" aria-label="運行の待ち・乗換試算">
  <h4>運行の待ち・乗換試算</h4>
  <p>現在のゲーム時刻から30分間の運行を再現し、競合待ちを含む鉄道の最早到着を調べます。同じ駅でのみ乗換、乗換余裕は1分です。時刻・需要・セーブは変更しません。</p>
  {stations.length<2?<p>異なる駅を2つ作成してください。</p>:<form onSubmit={event=>void calculate(event)}>
   <fieldset disabled={!enabled||pending}>
    {select('乗車駅',origin,setOrigin)}{select('降車駅',destination,setDestination)}
    <button type="submit" disabled={!valid}>運行を試算</button>
   </fieldset>
  </form>}
  {pending&&<p role="status">運行を試算中…</p>}
  {typeof value==='string'?<p role="alert">{value}</p>:value&&<>
   <p>試算時刻: {clock(value.ready_ms)} 〜 {clock(value.observed_until_ms)}（ワールド経過時刻）</p>
   {value.status==='incomplete'?<p role="status">計算上限に達しました。到着できるかは未確定です。</p>
   :value.status==='no_arrival_within_window'?<p role="status">試算した30分以内に到着する経路は見つかりませんでした。以降の運行は未確認です。</p>
   :<>
    <p>到着 {clock(value.arrival_ms!)} / 駅での待ち {clock(value.platform_wait_ms)} / 車内 {clock(value.onboard_ms)} / 乗換 {value.rides.length-1}回</p>
    <ol aria-label="試算した乗車経路">{value.rides.map((ride,i)=><li key={i}>
     {clock(ride.departed_ms)} {names.get(ride.from)??'駅'} → {clock(ride.arrived_ms)} {names.get(ride.to)??'駅'}（{serviceNames.get(ride.service_id)??'系統'}）
    </li>)}</ol>
   </>}
  </>}
 </section>;
}
