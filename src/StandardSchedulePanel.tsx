import {invoke} from '@tauri-apps/api/core';
import {useEffect,useRef,useState,type FormEvent} from 'react';
import {StandardRailFrequencyFields,frequencyBands,parseFrequencyDraft,type FrequencyBand,type FrequencyDraft} from './StandardRailFrequencyFields';

type Overrides=Record<FrequencyBand,number|null>;
export type ApplyStandardSchedule={service:string;expected:string;baseMinutes:number;overrides:Overrides;boundariesMinutes:number[]};
type Setup={baseMinutes:number;overrides:Overrides;boundariesMinutes:number[];startMs:number;endMs:number};
type Stop={id:string;name:string};
type Status={configurationToken:string;serviceId:string;canApply:boolean;activeElsewhere:boolean;hasBusConfigs?:boolean;authored:Setup|null;departureCount:number|null;origin:Stop|null;destinations:Stop[]};
type Od={status:'scheduled'|'no_departure_in_window'|'unverified_schedule';first_departure_ms:number|null;scheduled_departures:number|null;
 assignment:{rail:{trips:number;time:{wait_seconds:number;ride_seconds:number}}[];alternative_trips:number;unserved_trips:number;unresolved_trips:number}};
const minute=60_000,day=1440*minute;
const provisional=['0','360','600','960','1200','1440'];
function clock(ms:number){return `${Math.floor(ms/day)+1}日目 ${clockMinutes(Math.floor(ms%day/minute))}`;}
function clockMinutes(m:number){return `${String(Math.floor(m/60)).padStart(2,'0')}:${String(m%60).padStart(2,'0')}`;}
function number(raw:string,min:number,max:number){const n=Number(raw);return /^[0-9]+$/.test(raw)&&Number.isSafeInteger(n)&&n>=min&&n<=max?n:null;}
const bandNames=[...frequencyBands.map(row=>row[1]),'深夜の終了'];

export function StandardSchedulePanel({service,revision,operationToken,enabled,canApply,apply}:{service:string;revision:string;operationToken:string;enabled:boolean;canApply:boolean;apply:(args:ApplyStandardSchedule)=>Promise<void>}){
 const [status,setStatus]=useState<Status|null>(null),[error,setError]=useState('');
 const [draft,setDraft]=useState<FrequencyDraft>({baseMinutes:'10',overrides:{}});
 const [bounds,setBounds]=useState<string[]>(provisional);
 useEffect(()=>{
  let current=true;
  invoke<Status>('standard_schedule_status',{service,revision}).then(value=>{
   if(current&&value.configurationToken===revision&&value.serviceId===service)setStatus(value);
  }).catch(()=>{if(current)setError('時間帯ダイヤの状態を取得できませんでした。');});
  return()=>{current=false;};
 },[service,revision]);
 function save(event:FormEvent){
  event.preventDefault();if(!enabled||!canApply||!status?.canApply)return;
  setError('');
  const parsed=parseFrequencyDraft(draft);
  const boundsMinutes=bounds.map(b=>number(b,0,2879));
  if(!parsed||boundsMinutes.some((b,i)=>b===null||(i>0&&b<=(boundsMinutes[i-1]??Infinity)))||
   (boundsMinutes[0]??Infinity)>=1440||(boundsMinutes[5]??Infinity)>(boundsMinutes[0]??0)+1440){
   setError('運転間隔と境界を確認してください。日をまたぐ境界は1440分以上で入力できます。');return;
  }
  const overrides=Object.fromEntries(frequencyBands.map(([id])=>[id,parsed.overrides[id]??null])) as Overrides;
  void apply({service,expected:operationToken,baseMinutes:parsed.baseMinutes,overrides,boundariesMinutes:boundsMinutes as number[]});
 }
 return <section aria-label="標準の時間帯ダイヤ">
  <h5>時間帯ダイヤ</h5>
  {error&&<p role="alert">{error}</p>}
  {status?.hasBusConfigs&&(status.canApply||status.authored)&&<p className="field-hint">既存のバスダイヤはそのままです。バスと時間帯ダイヤを組み合わせたセーブは旧0.2.1では開けません。適用前のセーブを別の場所に残してください。</p>}
  {status?.authored?<><p>この系統の起点発車：{clock(status.authored.startMs)} ～ {clock(status.authored.endMs)}（終了時刻を含まない）。有効期間内に{status.departureCount}本。終了後に自動で延長しません。</p><p>従来のダイヤに戻すには時刻欄の「運行をリセット」を使います。時刻、運行中の車両、鉄道・バスの全ダイヤが消去されます。線路・駅・停留所・路線・系統と固定OD設定は残ります。</p>
   <ul>{frequencyBands.map(([id,label],i)=><li key={id}>{label} {clockMinutes(status.authored!.boundariesMinutes[i])}～{clockMinutes(status.authored!.boundariesMinutes[i+1])}：{status.authored!.overrides[id]??status.authored!.baseMinutes}分おき</li>)}</ul>
   <p>表示は設定ダイヤです。遅れや実際の輸送人数は含みません。</p>
   {status.origin&&status.destinations.length>0&&<StandardOdForm status={status} enabled={enabled}/>}</>:
  status?.activeElsewhere?<p>別の系統が時間帯ダイヤを使用中です。この検証版では同時に1系統までです。</p>:
  status?.canApply?<form onSubmit={save}>
   <p>基本の間隔と、必要なら朝夕などの本数を調整できます。適用後、少なくとも1分先の分境界から24ゲーム時間だけ有効です。曜日・祝日で変わりません。</p>
   <StandardRailFrequencyFields draft={draft} onChange={setDraft} disabled={!enabled}/>
   <details><summary>時間帯の境界（検証用の仮設定）</summary><p>ゲーム内の0時からの分数。既定の境界は仮設定で、現実の時刻表に基づきません。指定の外は基本間隔です。</p>
    {bounds.map((value,i)=><label key={i}>{bandNames[i]}（分）<input type="number" min="0" max="2879" step="1" value={value} onChange={e=>setBounds(old=>old.map((b,j)=>j===i?e.target.value:b))}/></label>)}
   </details>
   {!canApply&&<p>従来方式で運行を一度開始し、全車両の運行が終わったところで一時停止すると適用できます。</p>}
   <button type="submit" disabled={!enabled||!canApply}>時間帯ダイヤを適用</button>
  </form>:status?<p>この系統ではまだ利用できません。対応する往復系統を従来方式で設定してください。</p>:!error&&<p>利用条件を確認中…</p>}
 </section>;
}

function StandardOdForm({status,enabled}:{status:Status;enabled:boolean}){
 const setup=status.authored!;
 const [destination,setDestination]=useState(status.destinations[0].id);
 const [trips,setTrips]=useState('100'),[alternative,setAlternative]=useState(''),[offset,setOffset]=useState('0'),[duration,setDuration]=useState('60');
 const [result,setResult]=useState<Od|null>(null),[error,setError]=useState(''),[busy,setBusy]=useState(false);
 const revision=useRef(0),latestEnabled=useRef(enabled);latestEnabled.current=enabled;
 useEffect(()=>()=>{revision.current++;},[]);
 function changed(set:(v:string)=>void,value:string){revision.current++;set(value);setResult(null);setError('');}
 async function run(event:FormEvent){
  event.preventDefault();if(!enabled||busy)return;
  const token=++revision.current;setResult(null);setError('');
  const after=number(offset,0,1440),length=number(duration,1,1440),amount=number(trips,0,1_000_000_000);
  const other=alternative===''?null:number(alternative,1,1440);
  if(after===null||length===null||amount===null||(alternative!==''&&other===null)){setError('人数と試算範囲を確認してください。');return;}
  const start=setup.startMs+after*minute;
  setBusy(true);
  try{
   const response=await invoke<Od>('preview_standard_schedule_od',{service:status.serviceId,revision:status.configurationToken,
    query:{origin:status.origin!.id,destination,trips:amount,alternativeSeconds:other===null?null:other*60,startMs:start,endMs:start+length*minute}});
   if(revision.current===token&&latestEnabled.current)setResult(response);
  }catch{if(revision.current===token&&latestEnabled.current)setError('固定ODを試算できませんでした。往路の停車駅と時間を確認してください。');}
  finally{setBusy(false);}
 }
 const row=result?.assignment.rail[0];
 return <form onSubmit={run}><h6>固定OD試算（設定ダイヤ・仮の人数）</h6>
  <p>起点：{status.origin?.name}。往路の停車駅までの直通と、指定した他の移動方法を比較します。</p>
  <fieldset disabled={!enabled||busy}>
   <label>到着駅<select value={destination} onChange={e=>changed(setDestination,e.target.value)}>{status.destinations.map(s=><option key={s.id} value={s.id}>{s.name}</option>)}</select></label>
   <label>仮の人数<input type="number" min="0" value={trips} onChange={e=>changed(setTrips,e.target.value)}/></label>
   <label>他の移動方法の所要時間（分・任意）<input type="number" min="1" value={alternative} onChange={e=>changed(setAlternative,e.target.value)}/></label>
   <label>適用開始から何分後に駅で待つか<input type="number" min="0" value={offset} onChange={e=>changed(setOffset,e.target.value)}/></label>
   <label>発車を調べる時間（分）<input type="number" min="1" max="1440" value={duration} onChange={e=>changed(setDuration,e.target.value)}/></label>
   <button type="submit">この時間の固定ODを試算</button>
  </fieldset>
  {error&&<p role="alert">{error}</p>}
  {result&&enabled&&<div role="status">{result.status==='unverified_schedule'?'指定範囲のダイヤが未確認です。発車ゼロとは扱いません。':
   result.status==='no_departure_in_window'?'指定した範囲内に発車枠がありません。':`範囲内の発車枠：${result.scheduled_departures}本、最初：${clock(result.first_departure_ms!)}`}
   {row&&<p>鉄道 {row.trips.toFixed(1)}人、待ち {Math.round(row.time.wait_seconds)}秒、乗車 {Math.round(row.time.ride_seconds)}秒</p>}
   <p>別の移動方法 {result.assignment.alternative_trips.toFixed(1)}人、移動なし {result.assignment.unserved_trips.toFixed(1)}人、未確定 {result.assignment.unresolved_trips.toFixed(1)}人</p>
  </div>}
 </form>;
}
