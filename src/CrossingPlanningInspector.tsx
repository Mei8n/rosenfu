import {useRef,useState,type FormEvent} from 'react';
import {invoke} from '@tauri-apps/api/core';

const hour=3_600_000n, day=24n*hour, maxU64=(1n<<64n)-1n, visibleLimit=25;
type Pair=[string,string];
type Row={alignment_ids:Pair;approach_alignment_id:string;towards_alignment_end:boolean;scheduled_passages:[number,number];requested_utilization:number;estimated_delay_seconds_per_train:number};
type View={configuration_token:string;start_ms:string;end_ms:string;available:boolean;estimate_count:number;estimates:Row[];incomplete_count:number;incomplete_crossings:Pair[];sparse_count:number;sparse_crossings:Pair[]};
const exactDecimal=(x:unknown):x is string=>typeof x==='string'&&/^(0|[1-9][0-9]{0,19})$/.test(x)&&BigInt(x)<=maxU64;
const rowPair=(x:unknown):x is Pair=>Array.isArray(x)&&x.length===2&&x.every(exactDecimal)&&BigInt(x[0])<BigInt(x[1]);
const nonnegative=(x:unknown):x is number=>typeof x==='number'&&Number.isFinite(x)&&x>=0;
const count=(x:unknown):x is number=>nonnegative(x)&&Number.isSafeInteger(x);
function row(x:unknown):x is Row{
 const a=x as Partial<Row>|null;
 return !!a&&rowPair(a.alignment_ids)&&exactDecimal(a.approach_alignment_id)&&a.alignment_ids.includes(a.approach_alignment_id)
  &&typeof a.towards_alignment_end==='boolean'&&Array.isArray(a.scheduled_passages)&&a.scheduled_passages.length===2
  &&a.scheduled_passages.every(count)&&nonnegative(a.requested_utilization)&&nonnegative(a.estimated_delay_seconds_per_train);
}
function decode(raw:unknown,configuration:string,start:string):View|null{
 const r=raw as Partial<View>|null;
 const list=(total:unknown,rows:unknown,ok:(v:unknown)=>boolean)=>count(total)&&Array.isArray(rows)&&rows.length===Math.min(total,visibleLimit)&&rows.every(ok);
 if(!r||r.configuration_token!==configuration||r.start_ms!==start||r.end_ms!==(BigInt(start)+hour).toString()
  ||typeof r.available!=='boolean'||!list(r.estimate_count,r.estimates,row)||!list(r.incomplete_count,r.incomplete_crossings,rowPair)
  ||!list(r.sparse_count,r.sparse_crossings,rowPair)||(!r.available&&(r.estimate_count||r.incomplete_count||r.sparse_count))){
  return null;
 }
 return r as View;
}
// These are display-local labels, not track IDs, geographic bearings or
// railway directions. Keep wire identities for matching and React keys only.
const siteKey=(pair:Pair)=>pair.join(':');
function siteLabels(view:View|undefined){
 const labels=new Map<string,string>();
 if(!view)return labels;
 for(const pair of [...view.estimates.map(row=>row.alignment_ids),...view.incomplete_crossings,...view.sparse_crossings]){
  const key=siteKey(pair);
  if(!labels.has(key))labels.set(key,`交差 ${labels.size+1}`);
 }
 return labels;
}
function approachLabel(row:Row){
 const side=row.approach_alignment_id===row.alignment_ids[0]?0:2;
 return 'ABCD'[side+Number(row.towards_alignment_end)];
}
function startTime(d:string,t:string){
 if(!exactDecimal(d)||d==='0'||!/^([01][0-9]|2[0-3]):[0-5][0-9]$/.test(t))return null;
 const start=(BigInt(d)-1n)*day+BigInt(t.slice(0,2))*hour+BigInt(t.slice(3))*60_000n;
 return start+hour<=maxU64?start.toString():null;
}
function label(ms:string){
 const value=BigInt(ms);
 const clock=(n:bigint)=>String(n).padStart(2,'0');
 return `${value/day+1n}日目 ${clock(value%day/hour)}:${clock(value%hour/60_000n)}`;
}

export function CrossingPlanningInspector({enabled,configurationToken,nowMs}:{enabled:boolean;configurationToken?:string;nowMs:number}){
 const [dayInput,setDay]=useState(()=>String(Math.floor(nowMs/86_400_000)+1));
 const [timeInput,setTime]=useState(()=>`${String(Math.floor(nowMs/3_600_000)%24).padStart(2,'0')}:00`);
 const [reply,setReply]=useState<{scope:object;view?:View;error?:string}|null>(null);
 const [busy,setBusy]=useState<object|null>(null);
 const start=startTime(dayInput,timeInput);
 const stamp=useRef({enabled,configurationToken,start,scope:{}});
 if(stamp.current.enabled!==enabled||stamp.current.configurationToken!==configurationToken||stamp.current.start!==start){
  stamp.current={enabled,configurationToken,start,scope:{}};
 }
 const active=useRef<object|null>(null);
 const scope=stamp.current.scope;
 const current=reply?.scope===scope?reply:null;
 async function submit(event:FormEvent<HTMLFormElement>){
  event.preventDefault();
  if(!enabled||!configurationToken||start===null||active.current!==null)return;
  active.current=scope;setBusy(scope);setReply(null);
  try{
   const raw=await invoke<unknown>('preview_crossing_planning',{expectedConfigurationToken:configurationToken,startMs:start});
   if(stamp.current.scope===scope&&active.current===scope){
    const view=decode(raw,configurationToken,start);
    setReply(view?{scope,view}:{scope,error:'集計結果のワールドまたは時間帯を確認できませんでした。'});
   }
  }catch{
   // Never present internal command errors (or identifiers embedded in them).
   if(stamp.current.scope===scope&&active.current===scope)setReply({scope,error:'試算を取得できませんでした。最新の運行設定で調べ直してください。'});
  }finally{
   if(active.current===scope){active.current=null;setBusy(null);}
  }
 }
 const result=current?.view;
 const labels=siteLabels(result);
 const site=(pair:Pair)=>labels.get(siteKey(pair));
 return <details className="operation-list">
  <summary>平面交差の計画試算</summary>
  <p className="field-hint">通常線の一部が対象です。現在の設定ダイヤだけを使う試算で、次の実際の発車・通過時刻は予測しません。支障がない場合の通過時刻を基に計算し、実際の走行・乗降・需要には反映しません。</p>
  <form onSubmit={submit}>
   <label>開始日（ゲーム内）<input inputMode="numeric" maxLength={20} value={dayInput} onChange={e=>setDay(e.target.value)}/></label>
   <label>開始時刻<input type="time" value={timeInput} onChange={e=>setTime(e.target.value)}/></label>
   <button type="submit" disabled={!enabled||!configurationToken||start===null||busy!==null}>1時間を試算</button>
  </form>
  {start===null?<p className="field-hint">開始日と時刻を確認してください。</p>:<p className="field-hint">{label(start)} から {label((BigInt(start)+hour).toString())} まで（終了時刻は含まない）</p>}
  {busy!==null&&<p role="status">{busy===scope?'試算中…':'前の試算の完了を待っています…'}</p>}
  {current?.error&&<p role="alert">{current.error}</p>}
  {result&&!result.available&&<p role="status">利用できる鉄道ダイヤがありません。</p>}
  {result?.available&&<section aria-label="平面交差の試算結果">
   <p role="status">試算対象の進路 {result.estimate_count}件 / 計算できない交差 {result.incomplete_count}箇所 / 本数が少なく試算対象外 {result.sparse_count}箇所</p>
   {!result.estimate_count&&<p>この時間帯に試算できる進路はありません。待ち時間がゼロという意味ではありません。</p>}
   {result.estimate_count>result.estimates.length&&<p>平均遅れの大きい順に{result.estimates.length}件を表示</p>}
   <ul>{result.estimates.map(r=><li key={`${r.alignment_ids.join(':')}:${r.approach_alignment_id}:${r.towards_alignment_end}`}>
    {site(r.alignment_ids)}・進路 {approachLabel(r)}：設定ダイヤでの通過 {r.scheduled_passages[0]}本 / 交差側 {r.scheduled_passages[1]}本、見込み負荷 {(r.requested_utilization*100).toFixed(1)}%、平均遅れ {r.estimated_delay_seconds_per_train.toFixed(1)}秒/本
   </li>)}</ul>
   {result.incomplete_count>0&&<p>計算できない交差：{result.incomplete_crossings.map(site).join('、')}{result.incomplete_count>result.incomplete_crossings.length?' ほか':''}</p>}
   {result.sparse_count>0&&<p>本数が少ない交差：{result.sparse_crossings.map(site).join('、')}{result.sparse_count>result.sparse_crossings.length?' ほか':''}</p>}
  </section>}
 </details>;
}
