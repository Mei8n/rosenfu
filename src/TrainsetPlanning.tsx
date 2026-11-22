import {useEffect,useRef,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';

type Mode='timedHeadway'|'trainsetCount';
type Plan={cycleMs:number;runningMs:number;dwellAndClearanceMs:number;headwayMs:number;trainsets:number};
const whole=(value:number)=>Number.isSafeInteger(value)&&value>0;
function duration(ms:number){
 const minutes=Math.floor(ms/60_000),seconds=(ms%60_000/1000).toFixed(3).replace(/\.?0+$/,'');
 return `${minutes}分${seconds}秒`;
}

export function TrainsetPlanning({serviceId,configurationToken,currentHeadwayMs,enabled}:{
 serviceId:string;configurationToken:string;currentHeadwayMs:number;enabled:boolean;
}){
 const [mode,setMode]=useState<Mode>('timedHeadway'),[minutes,setMinutes]=useState(String(currentHeadwayMs/60_000)),[trainsets,setTrainsets]=useState('2');
 const [pending,setPending]=useState(false),[plan,setPlan]=useState<Plan>(),[error,setError]=useState('');
 const serial=useRef(0),flight=useRef(false);
 const latest=useRef({serviceId,configurationToken,enabled});latest.current={serviceId,configurationToken,enabled};
 function clear(){serial.current++;flight.current=false;setPending(false);setPlan(undefined);setError('');}
 useEffect(()=>{clear();setMinutes(String(currentHeadwayMs/60_000));return ()=>{serial.current++;};},
  [serviceId,configurationToken,currentHeadwayMs,enabled]);
 async function preview(){
  if(!enabled||flight.current)return;
  const value=mode==='timedHeadway'?Math.round(Number(minutes)*60_000):Number(trainsets);
  if(!whole(value)||mode==='trainsetCount'&&value>65_535){setError('運転間隔または編成数を正の値で指定してください。');return;}
  const request=mode==='timedHeadway'?{mode,headwayMs:value}:{mode,trainsets:value};
  const key=++serial.current;
  flight.current=true;setPending(true);setPlan(undefined);setError('');
  const alive=()=>serial.current===key&&latest.current.enabled&&latest.current.serviceId===serviceId&&latest.current.configurationToken===configurationToken;
  try{
   const reply=await invoke<Plan>('preview_nominal_trainsets',{serviceId,configurationToken,request});
   if(!alive())return;
   if(!reply||![reply.cycleMs,reply.runningMs,reply.headwayMs,reply.trainsets].every(whole)||
    !Number.isSafeInteger(reply.dwellAndClearanceMs)||reply.dwellAndClearanceMs<0||
    reply.runningMs+reply.dwellAndClearanceMs!==reply.cycleMs||
    (mode==='timedHeadway'?reply.headwayMs!==value:reply.trainsets!==value))throw Error('試算結果を確認できませんでした。');
   setPlan(reply);
  }catch(cause){if(alive())setError(typeof cause==='string'?cause:cause instanceof Error?cause.message:'試算できませんでした。');}
  finally{if(serial.current===key){flight.current=false;setPending(false);}}
 }
 return <details className="trainset-planning">
  <summary>編成数と運転間隔の試算</summary>
  <p>設定済みの経路・車両・停車時間で、他の列車との競合がない場合の目安です。ダイヤや運行は変更しません。</p>
  <fieldset disabled={!enabled||pending}><legend>試算条件</legend>
   <label>試算の基準<select value={mode} onChange={event=>{setMode(event.target.value as Mode);clear();}}>
    <option value="timedHeadway">運転間隔から必要編成数</option><option value="trainsetCount">編成数から運転間隔</option>
   </select></label>
   {mode==='timedHeadway'?<label>試算する運転間隔（分）<input type="number" min={1/60_000} step="any" value={minutes}
      onChange={event=>{setMinutes(event.target.value);clear();}}/></label>:
    <label>試算する編成数<input type="number" min="1" max="65535" step="1" value={trainsets}
      onChange={event=>{setTrainsets(event.target.value);clear();}}/></label>}
   <button type="button" onClick={()=>void preview()}>編成数・間隔を試算</button>
  </fieldset>
  {pending&&<p role="status">往復時間を確認中…</p>}
  {error&&<p role="alert">{error}</p>}
  {plan&&<dl>
   <dt>競合なしの往復時間</dt><dd>{duration(plan.cycleMs)}</dd>
   <dt>走行時間</dt><dd>{duration(plan.runningMs)}</dd>
   <dt>停車・折り返し・後端通過</dt><dd>{duration(plan.dwellAndClearanceMs)}</dd>
   {mode==='timedHeadway'?<><dt>必要な編成数の目安</dt><dd>{plan.trainsets}編成</dd></>:
    <><dt>最短運転間隔の目安</dt><dd>{duration(plan.headwayMs)}</dd></>}
  </dl>}
 </details>;
}
