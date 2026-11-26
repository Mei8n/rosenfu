import {invoke} from '@tauri-apps/api/core';
import {useEffect,useRef,useState,type FormEvent} from 'react';
import {StandardRailFrequencyFields,frequencyBands,parseFrequencyDraft,TimeBandFleetPlanner,type FrequencyBand,type FrequencyDraft} from './StandardRailFrequencyFields';

type Overrides=Record<FrequencyBand,number|null>;
export type ApplyStandardSchedule={service:string;expected:string;baseMinutes:number;overrides:Overrides;boundariesMinutes:number[];activeDays?:number};
type Setup={baseMinutes:number;overrides:Overrides;boundariesMinutes:number[];startMs:number;endMs:number};
type Stop={id:string;name:string};
type Status={configurationToken:string;serviceId:string;canApply:boolean;activeElsewhere:boolean;nominalCycleMs?:number|null;hasBusConfigs?:boolean;authored:Setup|null;departureCount:number|null;origin:Stop|null;destinations:Stop[]};
type Od={status:'scheduled'|'no_departure_in_window'|'unverified_schedule';first_departure_ms:number|null;scheduled_departures:number|null;
 assignment:{rail:{trips:number;time:{wait_seconds:number;ride_seconds:number}}[];alternative_trips:number;unserved_trips:number;unresolved_trips:number}};
const minute=60_000,day=1440*minute;
const provisional=['0','360','600','960','1200','1440'];
function clock(ms:number){return `${Math.floor(ms/day)+1}日目 ${clockMinutes(Math.floor(ms%day/minute))}`;}
function clockMinutes(m:number){return `${String(Math.floor(m/60)).padStart(2,'0')}:${String(m%60).padStart(2,'0')}`;}
function number(raw:string,min:number,max:number){const n=Number(raw);return /^[0-9]+$/.test(raw)&&Number.isSafeInteger(n)&&n>=min&&n<=max?n:null;}
const bandNames=[...frequencyBands.map(row=>row[1]),'深夜の終了'];

export function StandardSchedulePanel({service,revision,operationToken,currentHeadwayMs,originalLastDepartureMs,nowMs,enabled,canApply,applyBlocker,apply,restore}:{service:string;revision:string;operationToken:string;currentHeadwayMs?:number;originalLastDepartureMs?:number;nowMs?:number;enabled:boolean;canApply:boolean;applyBlocker?:string|null;apply:(args:ApplyStandardSchedule)=>Promise<void>;restore?:(args:{service:string;expected:string})=>Promise<void>}){
 const [loadedStatus,setStatus]=useState<Status|null>(null),[error,setError]=useState('');
 const status=loadedStatus?.serviceId===service&&loadedStatus.configurationToken===revision?loadedStatus:null;
 const currentMinutes=(currentHeadwayMs??0)/minute;
 const inheritedBase=Number.isInteger(currentMinutes)&&currentMinutes>=1&&currentMinutes<=240?String(currentMinutes):'';
 const [draft,setDraft]=useState<FrequencyDraft>({baseMinutes:inheritedBase,overrides:{}});
 const [bounds,setBounds]=useState<string[]>(provisional);
 const [editing,setEditing]=useState(false);
 const [activeDays,setActiveDays]=useState('1');
 const authored=status?.authored;
 function startEdit(){
  if(!authored)return;
  setDraft({baseMinutes:String(authored.baseMinutes),overrides:Object.fromEntries(
   frequencyBands.flatMap(([id])=>authored.overrides[id]===null?[]:[[id,String(authored.overrides[id])]]),
  )});
  setBounds(authored.boundariesMinutes.map(String));
  setActiveDays(String(Math.ceil((authored.endMs-authored.startMs)/day)));
  setError('');
  setEditing(true);
 }
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
  const days=number(activeDays,1,30);
  if(days===null){setError('有効期間は1〜30ゲーム日で指定してください。');return;}
  const parsed=parseFrequencyDraft(draft);
  const boundsMinutes=bounds.map(b=>number(b,0,2879));
  if(!parsed||boundsMinutes.some((b,i)=>b===null||(i>0&&b<=(boundsMinutes[i-1]??Infinity)))||
   (boundsMinutes[0]??Infinity)>=1440||(boundsMinutes[5]??Infinity)>(boundsMinutes[0]??0)+1440){
   setError('運転間隔と境界を確認してください。日をまたぐ境界は1440分以上で入力できます。');return;
  }
  const overrides=Object.fromEntries(frequencyBands.map(([id])=>[id,parsed.overrides[id]??null])) as Overrides;
  void apply({service,expected:operationToken,baseMinutes:parsed.baseMinutes,overrides,boundariesMinutes:boundsMinutes as number[],...(days===1?{}:{activeDays:days})});
 }
 return <section aria-label="標準の時間帯ダイヤ">
  <h5>時間帯ダイヤ</h5>
  {error&&<p role="alert">{error}</p>}
  {status?.hasBusConfigs&&(status.canApply||status.authored)&&<p className="field-hint">既存のバスダイヤはそのままです。バスと時間帯ダイヤを組み合わせたセーブは旧0.2.1では開けません。適用前のセーブを別の場所に残してください。</p>}
  {status&&authored&&<>
   <p>この系統の起点発車：{clock(authored.startMs)} ～ {clock(authored.endMs)}（終了時刻を含まない）。有効期間内に{status.departureCount}本。終了後に自動で延長しません。</p>
   <ul>{frequencyBands.map(([id,label],i)=><li key={id}>{label} {clockMinutes(authored.boundariesMinutes[i])}～{clockMinutes(authored.boundariesMinutes[i+1])}：{authored.overrides[id]??authored.baseMinutes}分おき</li>)}</ul>
   <p>表示は設定ダイヤです。遅れや実際の輸送人数は含みません。</p>
   {status.origin&&status.destinations.length>0&&<StandardOdForm status={status} stops={[status.origin,...status.destinations]} enabled={enabled}/>}
   {!editing&&status.canApply&&<button type="button" disabled={!enabled} onClick={startEdit}>時間帯ダイヤを変更</button>}
  </>}
  {status?.activeElsewhere?<p>別の系統が時間帯ダイヤを使用中です。この検証版では同時に1系統までです。</p>:
  status?.canApply&&(!authored||editing)?<form onSubmit={save}>
   <p>基本の間隔と、必要なら朝夕などの本数を調整できます。適用後、少なくとも1分先の分境界から有効になり、指定日数だけ同じ時間帯を繰り返します。曜日・祝日で変わりません。</p>
   {!authored&&!inheritedBase&&<p>従来ダイヤの間隔をそのまま引き継げないため、基本の間隔を1～240分の整数で入力してください。</p>}
   <StandardRailFrequencyFields draft={draft} onChange={setDraft} disabled={!enabled}/>
   <TimeBandFleetPlanner cycleMs={status.nominalCycleMs} disabled={!enabled} onAdopt={(target,minutes)=>{
    setDraft(old=>target==='base'?{...old,baseMinutes:String(minutes)}:{...old,overrides:{...old.overrides,[target]:String(minutes)}});
    setError('');
   }}/>
   <label>有効期間（ゲーム日）<input type="number" min="1" max="30" step="1" required disabled={!enabled} value={activeDays} onChange={e=>setActiveDays(e.target.value)}/></label>
   <details><summary>時間帯の境界（検証用の仮設定）</summary><p>ゲーム内の0時からの分数。既定の境界は仮設定で、現実の時刻表に基づきません。指定の外は基本間隔です。</p>
    {bounds.map((value,i)=><label key={i}>{bandNames[i]}（分）<input type="number" min="0" max="2879" step="1" value={value} onChange={e=>setBounds(old=>old.map((b,j)=>j===i?e.target.value:b))}/></label>)}
   </details>
   {authored?<p>この系統の今後の発車だけを置き換えます。新しい時間帯ダイヤの開始までにあった旧ダイヤの便も運行しません。他の系統や時刻はそのままです。</p>:
    <p>この系統の従来ダイヤを置き換えます。運行開始前は、この鉄道系統だけにダイヤがあれば適用できます。</p>}
   {!canApply&&<p role="status">{applyBlocker??'車両がいない状態で運行を一時停止してください。'}</p>}
   <button type="submit" disabled={!enabled||!canApply}>{authored?'変更を適用':'時間帯ダイヤを適用'}</button>
   {authored&&<button type="button" onClick={()=>{setEditing(false);setError('');}}>変更を取り消す</button>}
  </form>:status&&!authored?<p>この系統ではまだ利用できません。対応する往復系統を従来方式で設定してください。</p>:!status&&!error&&<p>利用条件を確認中…</p>}
  {authored&&restore&&<div>
   <p>元の簡易ダイヤに戻すと、この系統の今後の発車だけが変わります。時刻や他の系統のダイヤはそのままです。</p>
   {nowMs!==undefined&&originalLastDepartureMs!==undefined&&originalLastDepartureMs<=nowMs&&<p>元の終発は過ぎています。戻した後に、簡易ダイヤの発車時刻を設定し直してください。</p>}
   {!canApply&&!editing&&<p role="status">{applyBlocker??'車両がいない状態で運行を一時停止してください。'}</p>}
   <button type="button" disabled={!enabled||!canApply||editing} onClick={()=>void restore({service,expected:operationToken})}>元の簡易ダイヤに戻す</button>
  </div>}
 </section>;
}

function StandardOdForm({status,stops,enabled}:{status:Status;stops:Stop[];enabled:boolean}){
 const setup=status.authored!;
 const horizonMinutes=Math.ceil((setup.endMs-setup.startMs)/minute);
 const [origin,setOrigin]=useState(stops[0].id),[destination,setDestination]=useState(stops[1].id);
 const destinations=stops.slice(stops.findIndex(stop=>stop.id===origin)+1);
 const [trips,setTrips]=useState('100'),[alternative,setAlternative]=useState(''),[offset,setOffset]=useState('0'),[duration,setDuration]=useState('60');
 const [result,setResult]=useState<Od|null>(null),[error,setError]=useState(''),[busy,setBusy]=useState(false);
 const revision=useRef(0),latestEnabled=useRef(enabled);latestEnabled.current=enabled;
 useEffect(()=>()=>{revision.current++;},[]);
 function changed(set:(v:string)=>void,value:string){revision.current++;set(value);setResult(null);setError('');}
 async function run(event:FormEvent){
  event.preventDefault();if(!enabled||busy)return;
  const token=++revision.current;setResult(null);setError('');
  const after=number(offset,0,horizonMinutes),length=number(duration,1,1440),amount=number(trips,0,1_000_000_000);
  const other=alternative===''?null:number(alternative,1,1440);
  if(after===null||length===null||amount===null||(alternative!==''&&other===null)){setError('人数と試算範囲を確認してください。');return;}
  if(!destinations.some(stop=>stop.id===destination))return;
  const start=setup.startMs+after*minute;
  setBusy(true);
  try{
   const response=await invoke<Od>('preview_standard_schedule_od',{service:status.serviceId,revision:status.configurationToken,
    query:{origin,destination,trips:amount,alternativeSeconds:other===null?null:other*60,startMs:start,endMs:start+length*minute}});
   if(revision.current===token&&latestEnabled.current)setResult(response);
  }catch{if(revision.current===token&&latestEnabled.current)setError('固定ODを試算できませんでした。往路の乗車駅・到着駅と時刻を確認してください。');}
  finally{setBusy(false);}
 }
 const row=result?.assignment.rail[0];
 return <form onSubmit={run}><h6>固定OD試算（設定ダイヤ・仮の人数）</h6>
  <p>往路の停車駅間の直通と、指定した他の移動方法を比較します。途中駅の発車枠は、起点発車からの予定時分だけずれます。</p>
  <fieldset disabled={!enabled||busy}>
   <label>乗車駅<select value={origin} onChange={e=>{
    const next=e.target.value, later=stops.slice(stops.findIndex(stop=>stop.id===next)+1);
    changed(setOrigin,next);
    if(!later.some(stop=>stop.id===destination))setDestination(later[0].id);
   }}>{stops.slice(0,-1).map(s=><option key={s.id} value={s.id}>{s.name}</option>)}</select></label>
   <label>到着駅<select value={destination} onChange={e=>changed(setDestination,e.target.value)}>{destinations.map(s=><option key={s.id} value={s.id}>{s.name}</option>)}</select></label>
   <label>仮の人数<input type="number" min="0" value={trips} onChange={e=>changed(setTrips,e.target.value)}/></label>
   <label>他の移動方法の所要時間（分・任意）<input type="number" min="1" value={alternative} onChange={e=>changed(setAlternative,e.target.value)}/></label>
   <label>起点のダイヤ適用開始から何分後に乗車駅で待つか<input type="number" min="0" max={horizonMinutes} value={offset} onChange={e=>changed(setOffset,e.target.value)}/></label>
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
