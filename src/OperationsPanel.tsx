import {createContext,useContext,useEffect,useRef,useState,type FormEvent,type ReactNode} from 'react';
import {invoke} from '@tauri-apps/api/core';
import './operationsPanel.css';
import {TrainsetPlanning} from './TrainsetPlanning';
import {CrossingPlanningInspector} from './CrossingPlanningInspector';
import {OperationMapMarkers} from './OperationMapMarkers';
import type {Camera} from './map';

import {StandardSchedulePanel,type ApplyStandardSchedule} from './StandardSchedulePanel';

type Timetable={first_departure_ms:number;last_departure_ms:number;headway_ms:number};
type Vehicle={max_speed_kmh:number;acceleration_mps2:number;braking_mps2:number;length_m:number};
type Config={timetable:Timetable;vehicle:Vehicle;dwell_ms:number};
type ServiceConfig=Config&{service_id:string};
type BusConfig=Config&{route_id:string;assumed_speed_kmh:number;turnaround_ms?:number};
type Train={key:{service_id:string;scheduled_ms:number};leg:number;delay_ms:number;waiting_for?:'crossing'|'junction'|'platform'|'track';wait_scope?:'held_across_connections'|'needs_across_connections';state:{state:'waiting';ready_ms:number}|{state:'moving';entered_ms:number;arrival_ms:number;release_ms:number}};
export type OperationView={standard_schedule_active?:boolean;now_ms:number;paused:boolean;speed:number;invalidated:boolean;catching_up:boolean;timetable_edit_token:string;timetable_edit_blocker?:string|null;rail_timetable_handoff_available?:boolean;rail_timetable_handoff_in_progress?:boolean;configuration_token?:string;services:ServiceConfig[];bus_services?:BusConfig[];trains:Train[];rail_markers?:unknown;bus_markers?:unknown};
type OperationWire=Omit<OperationView,'services'|'bus_services'>&{services?:ServiceConfig[];bus_services?:BusConfig[]};
type Commands={apply_standard_schedule:ApplyStandardSchedule;reset_operation:undefined;set_operation_clock:{paused:boolean;speed:number};configure_operation_service:{serviceId:string;expectedOperationToken:string;timetable:Timetable;vehicle:Vehicle;dwellMs:number};configure_bus_operation:{routeId:string;expectedOperationToken:string;timetable:Timetable;vehicle:Vehicle;dwellMs:number;assumedSpeedKmh:number;turnaroundMs?:number}};
type OperationContext={view:OperationView|null;crossingPlanningToken?:string;busy:boolean;blocked:boolean;error:string;refresh:()=>Promise<void>;send:<K extends keyof Commands>(command:K,args:Commands[K])=>Promise<void>};
const Context=createContext<OperationContext|null>(null);
const speeds=[1,2,4,8,16] as const;
const minute=60_000,day=24*60*minute;
const defaultOpenBusTurnaroundMs=120_000;

export function OperationProvider({available,blocked,epoch,worldVersion,onChange,onRunningChange,acquireMutation,children}:{available:boolean;blocked:boolean;epoch:number;worldVersion?:unknown;onChange:()=>void;onRunningChange?:(running:boolean)=>void;acquireMutation?:()=>null|(()=>void);children:ReactNode}){
 const [snapshot,setSnapshot]=useState<{epoch:number;view:OperationView;worldVersion:unknown}|null>(null);
 const [busy,setBusy]=useState(false),[error,setError]=useState('');
 const generation=useRef(0),flight=useRef<{done:Promise<void>}|null>(null),intent=useRef<object|null>(null);
 const gate=useRef({available,blocked,epoch,worldVersion,onChange,acquireMutation});gate.current={available,blocked,epoch,worldVersion,onChange,acquireMutation};
 const view=snapshot?.epoch===epoch?snapshot.view:null;
 async function request(command:string,args?:Commands[keyof Commands],tick=false){
  const before=gate.current,token=generation.current;
  if(!before.available||before.blocked||(tick&&(flight.current||intent.current))||(!tick&&intent.current))return;
  const current=()=>token===generation.current&&before.epoch===gate.current.epoch&&gate.current.available;
  const userIntent={};
  if(!tick){intent.current=userIntent;setBusy(true);setError('');}
  let lock:{done:Promise<void>}|undefined;
  let release:()=>void=()=>{},releaseMutation:(()=>void)|null|undefined;
  try{
   // An explicit operation waits for a pending timer tick instead of being lost.
   if(flight.current)await flight.current.done;
   if(!current()||gate.current.blocked)return;
   if(!tick&&command!=='operation_snapshot'){releaseMutation=gate.current.acquireMutation?.();if(releaseMutation===null)return;}
   lock={done:new Promise<void>(resolve=>{release=resolve;})};flight.current=lock;
   const cached=tick&&snapshot?.epoch===before.epoch&&snapshot.worldVersion===before.worldVersion?snapshot.view:null;
   const known=cached?.configuration_token;
   const next=await invoke<OperationWire>(command,known?{knownConfigurationToken:known}:args);
   if(!current()||((tick||command==='operation_snapshot')&&before.worldVersion!==gate.current.worldVersion))return;
   const invalid=()=>Error('運行エンジンの応答を確認できませんでした。');
   if(!Number.isSafeInteger(next?.now_ms)||next.now_ms<0||typeof next.paused!=='boolean'||typeof next.timetable_edit_token!=='string'||!next.timetable_edit_token||!speeds.some(speed=>speed===next.speed)||!Array.isArray(next.trains)||(next.configuration_token!==undefined&&(typeof next.configuration_token!=='string'||!next.configuration_token)))throw invalid();
   let view:OperationView;
   if(Array.isArray(next.services)&&(next.bus_services===undefined||Array.isArray(next.bus_services))){
    view={...next,services:next.services};
   }else if(cached&&known===next.configuration_token&&known&&next.services===undefined&&next.bus_services===undefined){
    view={...next,services:cached.services,bus_services:cached.bus_services};
   }else throw invalid();
   setSnapshot({epoch:before.epoch,worldVersion:before.worldVersion,view});
   if(!tick&&command!=='operation_snapshot')gate.current.onChange();
  }catch(cause){
   if(current())setError(String(cause));
  }finally{
   if(flight.current===lock)flight.current=null;release();releaseMutation?.();
   if(intent.current===userIntent){intent.current=null;setBusy(false);}
  }
 }
 // A world's reply may never replace a newly loaded world's clock.
 useEffect(()=>{
  generation.current++;flight.current=null;intent.current=null;setBusy(false);setError('');
  return()=>{generation.current++;flight.current=null;intent.current=null;};
 },[available,epoch]);
 useEffect(()=>{if(available&&!blocked&&!busy&&(!view||snapshot?.worldVersion!==worldVersion)&&!error)void request('operation_snapshot');},[available,blocked,busy,epoch,view,error,worldVersion]);
 const running=available&&!!view&&!view.paused&&!view.invalidated;
 useEffect(()=>{onRunningChange?.(running);},[running,onRunningChange]);
 const timerEnabled=running&&!blocked&&!error;
 const tick=useRef(()=>request('advance_operation',undefined,true));
 tick.current=()=>request('advance_operation',undefined,true);
 useEffect(()=>{
  if(!timerEnabled)return;
  // Only the native monotonic clock decides elapsed game time.
  const id=window.setInterval(()=>{if(!flight.current&&!intent.current)void tick.current();},500);
  return()=>window.clearInterval(id);
 },[timerEnabled,epoch]);
 const state:OperationContext|null=available?{view,busy,blocked,error,crossingPlanningToken:snapshot?.worldVersion===worldVersion?view?.configuration_token:undefined,refresh:()=>request('operation_snapshot'),send:request}:null;
 return <Context.Provider value={state}>{children}</Context.Provider>;
}

function scheduleTime(ms:number){
 const within=ms%day,hours=Math.floor(within/(60*minute)),minutes=Math.floor(within/minute)%60;
 return `${Math.floor(ms/day)+1}日目 ${String(hours).padStart(2,'0')}:${String(minutes).padStart(2,'0')}`;
}

export function OperationObservationMinute({observedMs}:{observedMs:number}){
 const now=useContext(Context)?.view?.now_ms;
 const clockAdvanced=now!==undefined&&Math.floor(now/minute)>Math.floor(observedMs/minute);
 return <p>観測開始: {scheduleTime(observedMs)} <span role="status">{clockAdvanced?'時刻が進みました。最新の時刻で見るには再試算してください。':null}</span></p>;
}

export function OperationClock(){
 const state=useContext(Context);const [confirmReset,setConfirmReset]=useState(false);if(!state)return null;
 const {view,busy,blocked,error,refresh,send}=state;
 const disabled=busy||blocked;
 const hasConfigured=!!view&&(view.services.length>0||!!view.bus_services?.length);
 const clockDisabled=disabled||!!view?.invalidated||!hasConfigured;
 return <div className="operation-clock" role="group" aria-label="ゲーム内時刻">
  {view?<>
   <output aria-live="off">{scheduleTime(view.now_ms)}</output>
   <button type="button" className="secondary" disabled={clockDisabled} onClick={()=>void send('set_operation_clock',{paused:!view.paused,speed:view.speed})}>{view.paused?'再開':'一時停止'}</button>
   <label>速度<select value={view.speed} disabled={clockDisabled} onChange={e=>void send('set_operation_clock',{paused:view.paused,speed:Number(e.target.value)})}>{speeds.map(speed=><option key={speed} value={speed}>{speed}倍</option>)}</select></label>
   {(view.invalidated||view.now_ms>0||hasConfigured)&&<button type="button" className="secondary" disabled={disabled} onClick={()=>setConfirmReset(true)}>運行をリセット</button>}
   {view.invalidated&&<span role="status">路線または系統が変更され、運行を停止しました。{view.timetable_edit_blocker??"ダイヤを適用し直すと、時刻を保って再開できます。"}</span>}
   {view.rail_timetable_handoff_in_progress===true&&<span role="status">変更前のダイヤで発車した列車が運行中です。この間に保存したデータは旧ビルドでは開けません。</span>}
   {view.catching_up&&<span role="status">運行時刻に追いつく処理中</span>}
   {confirmReset&&<div className="operation-reset" role="group" aria-label="運行リセットの確認"><p>時刻と鉄道・バスの全ダイヤを消去します。線路・駅・停留所・路線・系統は残ります。</p><button type="button" className="danger" disabled={disabled} onClick={()=>{setConfirmReset(false);void send('reset_operation',undefined);}}>時刻とダイヤを消去</button><button type="button" className="secondary" onClick={()=>setConfirmReset(false)}>リセットを取り消す</button></div>}
  </>:<span>{error?'時刻を取得できません':'時刻を確認中…'}</span>}
  {error&&<><span role="alert">{error}</span><button type="button" className="secondary" disabled={disabled} onClick={()=>void refresh()}>運行状態を再取得</button></>}
 </div>;
}

export function OperationVehicleLayer({camera,scale}:{camera:Camera;scale:number}){
 const state=useContext(Context);
 const view=state?.view;
 if(!state?.crossingPlanningToken||state.blocked||!view||view.invalidated)return null;
 return <>
  <OperationMapMarkers markers={view.rail_markers} camera={camera} scale={scale}/>
  <OperationMapMarkers markers={view.bus_markers} camera={camera} scale={scale} mode="bus"/>
 </>;
}

export function CrossingOperations({enabled}:{enabled:boolean}){
 const state=useContext(Context);
 if(!state?.view)return null;
 if(state.view.standard_schedule_active)return <p>時間帯ダイヤの平面交差試算は未対応です。</p>;
 return <CrossingPlanningInspector enabled={enabled&&!state.blocked&&!state.busy} configurationToken={state.crossingPlanningToken} nowMs={state.view.now_ms}/>;
}

type Form={firstDay:string;firstTime:string;lastDay:string;lastTime:string;headway:string;dwell:string;maxSpeed:string;acceleration:string;length:string;assumedSpeed:string};
function departureFields(ms:number){
 const within=ms%day,h=Math.floor(within/(60*minute)),m=Math.floor(within/minute)%60,s=Math.floor(within/1000)%60;
 return {day:String(Math.floor(ms/day)+1),time:[h,m,s].map(v=>String(v).padStart(2,'0')).join(':')};
}
function initialForm(config:Config|BusConfig|undefined,now:number,bus:boolean):Form{
 const nextMinute=now===0?0:(Math.floor(now/minute)+1)*minute;
 const first=departureFields(config?.timetable.first_departure_ms??nextMinute);
 const last=departureFields(config?.timetable.last_departure_ms??nextMinute+18*60*minute);
 return {firstDay:first.day,firstTime:first.time,lastDay:last.day,lastTime:last.time,headway:String((config?.timetable.headway_ms??10*minute)/minute),dwell:String((config?.dwell_ms??30_000)/1000),maxSpeed:String(config?.vehicle.max_speed_kmh??(bus?80:100)),acceleration:String(config?.vehicle.acceleration_mps2??(bus?1:0.8)),length:String(config?.vehicle.length_m??(bus?12:80)),assumedSpeed:String(config&&"assumed_speed_kmh" in config?config.assumed_speed_kmh:30)};
}
function departure(dayValue:string,time:string){
 const d=Number(dayValue),parts=time.split(':').map(Number);
 if(!Number.isSafeInteger(d)||d<1||!/^\d{2}:\d{2}(?::\d{2})?$/.test(time)||parts[0]>23||parts[1]>59||(parts[2]??0)>59) return NaN;
 return (d-1)*day+parts[0]*60*minute+parts[1]*minute+(parts[2]??0)*1000;
}

function TimetableForm({serviceId,config,now,disabled,send,expectedOperationToken,bus=false,ordinaryRoundTrip=false,scheduleOnly=false}:{serviceId:string;expectedOperationToken:string;config?:Config|BusConfig;now:number;disabled:boolean;send:OperationContext['send'];bus?:boolean;ordinaryRoundTrip?:boolean;scheduleOnly?:boolean}){
 const [form,setForm]=useState(()=>initialForm(config,now,bus));
 const [validation,setValidation]=useState('');
 const newTurnaround=ordinaryRoundTrip&&!config?defaultOpenBusTurnaroundMs:undefined;
 const turnaround=config&&'turnaround_ms' in config?config.turnaround_ms:newTurnaround;
 const update=(key:keyof Form,value:string)=>{setForm(f=>({...f,[key]:value}));setValidation('');};
 function save(event:FormEvent<HTMLFormElement>){
  event.preventDefault();if(disabled)return;
  const start=departure(form.firstDay,form.firstTime),end=departure(form.lastDay,form.lastTime);
  const timetable={first_departure_ms:start,last_departure_ms:end,headway_ms:Math.round(Number(form.headway)*minute)};
  const vehicle=scheduleOnly&&config?config.vehicle:{max_speed_kmh:Number(form.maxSpeed),acceleration_mps2:Number(form.acceleration),braking_mps2:config?.vehicle.braking_mps2??(bus?1.2:1),length_m:Number(form.length)};
  const dwellMs=scheduleOnly&&config?config.dwell_ms:Math.round(Number(form.dwell)*1000);
  if(![start,end,timetable.headway_ms,dwellMs].every(Number.isSafeInteger)||end<start||timetable.headway_ms<=0||dwellMs<0||!Object.values(vehicle).every(n=>Number.isFinite(n)&&n>0)||!form.dwell.trim()){
   setValidation('発車時刻・運転間隔・車両性能を確認してください。終発は初発以降にしてください。');return;
  }
  if(bus){
   const assumedSpeedKmh=Number(form.assumedSpeed);
   if(!Number.isFinite(assumedSpeedKmh)||assumedSpeedKmh<1||assumedSpeedKmh>120){setValidation('シミュレーション用の走行速度は1〜120 km/hで入力してください。');return;}
   void send('configure_bus_operation',{routeId:serviceId,expectedOperationToken,timetable,vehicle,dwellMs,assumedSpeedKmh,...(newTurnaround===undefined?{}:{turnaroundMs:newTurnaround})});
  }else void send('configure_operation_service',{serviceId,expectedOperationToken,timetable,vehicle,dwellMs});
 }
 const number=(key:keyof Form,label:string,min:number,step:string|number='any',max?:number)=><label>{label}<input type="number" disabled={scheduleOnly&&(key==='dwell'||key==='maxSpeed'||key==='acceleration'||key==='length')} required min={min} max={max} step={step} value={form[key]} onChange={e=>update(key,e.target.value)}/></label>;
 return <form className="operation-form" onSubmit={save}><fieldset disabled={disabled}><legend>簡易ダイヤ</legend>
  <div className="operation-times">{number('firstDay','初発の日（何日目）',1,1)}<label>初発時刻<input type="time" required step="1" value={form.firstTime} onChange={e=>update('firstTime',e.target.value)}/></label>
  {number('lastDay','終発の日（何日目）',1,1)}<label>終発時刻<input type="time" required step="1" value={form.lastTime} onChange={e=>update('lastTime',e.target.value)}/></label></div>
  <div className="operation-times">{number('headway','運転間隔（分）',0.01)}{number('dwell',bus?'停留所の停車時間（秒）':'駅の停車時間（秒）',0)}</div>
  {turnaround!==undefined&&<p>終点の折り返し時間：{turnaround/1000}秒</p>}
  {bus&&<>{number('assumedSpeed','想定走行速度（km/h）',1,'any',120)}<p>ゲーム内の仮定値です。実際の道路の制限速度ではありません。</p></>}
  <details><summary>使用車両の性能</summary><div className="operation-times">{number('maxSpeed','最高速度（km/h）',1)}{number('acceleration','加速度（m/s²）',0.01)}{number('length',bus?'車両長（m）':'編成長（m）',1)}</div></details>
  {now>0&&departure(form.lastDay,form.lastTime)<=now&&<p>指定した終発時刻はすでに過ぎています。</p>}
  {validation&&<p role="alert">{validation}</p>}
  <button type="submit">{scheduleOnly?"今後の発車時刻を変更":config?'ダイヤの変更を適用':'運行を設定'}</button>
 </fieldset></form>;
}

export function ServicePathChangeNotice({serviceId}:{serviceId:string}){
 const state=useContext(Context);
 if(!state?.view?.services.some(row=>row.service_id===serviceId))return null;
 return <p className="muted">運行に影響する経路・停車駅の変更は、全車両の運行が終わってから一時停止して適用してください。元に戻す・やり直すときも同じ条件です。名前だけの変更は運行中でもできます。</p>;
}

export function ServiceOperations({serviceId,enabled,standardEnabled=false}:{serviceId:string;enabled:boolean;standardEnabled?:boolean}){
 const state=useContext(Context);if(!state||!state.view)return null;
 const {view,busy,blocked,send,error}=state;
 const config=view.services.find(row=>row.service_id===serviceId);
 const blocker=view.timetable_edit_blocker;
 const standardBlocker=view.invalidated?'線路や系統の変更で運行を停止しました。運行設定を確認してください。'
  :view.catching_up
  ?(view.paused?'運行を再開して時刻に追いついてから、一時停止してください。':'運行が時刻に追いついてから、一時停止してください。')
  :blocker??(!view.paused?'運行を一時停止してください。'
   :view.now_ms===0&&(view.services.length>1||!!view.bus_services?.length)
   ?'運行開始前は、この鉄道系統だけにダイヤを設定してください。複数のダイヤがある場合は、運行を開始し、車両がいないときに一時停止すると適用できます。'
   :null);
 const trains=view.trains.filter(train=>train.key.service_id===serviceId);
 const scheduleOnly=!!config&&view.paused&&!view.invalidated&&!view.catching_up&&view.rail_timetable_handoff_available===true;
 const advanced=<>
  <TimetableForm key={serviceId+':'+JSON.stringify(config??null)} serviceId={serviceId} config={config} now={view.now_ms} disabled={!enabled||busy||blocked||(!!blocker&&!scheduleOnly)} scheduleOnly={scheduleOnly} send={send} expectedOperationToken={view.timetable_edit_token}/>
  {scheduleOnly?<p>すでに発車した車両の運行はそのままです。現在より後の発車時刻だけ変更できます。</p>:blocker?<p>{blocker}</p>:view.now_ms>0&&<p>現在の時刻と他のダイヤはそのままです。変更後の過去の便は走りません。</p>}
  {config&&view.configuration_token&&<TrainsetPlanning serviceId={serviceId} configurationToken={view.configuration_token}
   currentHeadwayMs={config.timetable.headway_ms} enabled={enabled&&!blocked&&!view.invalidated}/>}
 </>;
 return <section className="service-operations" aria-label="系統の運行">
  <h4>運行</h4>
  {standardEnabled&&state.crossingPlanningToken&&<StandardSchedulePanel key={serviceId+':'+state.crossingPlanningToken}
   service={serviceId} revision={state.crossingPlanningToken} operationToken={view.timetable_edit_token}
   enabled={enabled&&!busy&&!blocked&&!view.invalidated}
   canApply={!!config&&!standardBlocker} applyBlocker={standardBlocker}
   apply={args=>send('apply_standard_schedule',args)}/>}
  {view.standard_schedule_active?<p>時間帯ダイヤを使用中のため、従来のダイヤ編集と編成試算は利用できません。</p>:
   standardEnabled?<details><summary>時刻・車両の設定（従来方式）</summary>{advanced}</details>:advanced}
  <ActiveVehicles trains={trains} configured={!!config} now={view.now_ms} invalidated={view.invalidated}/>
  {error&&<p role="status">時刻欄の運行エラーを確認してください。</p>}
 </section>;
}

function vehicleStatus(train:Train,now:number,invalidated:boolean,bus:boolean){
 const {state}=train;
 if(invalidated)return {status:'運行停止中',delayMs:train.delay_ms};
 if(bus)return {status:state.state==='waiting'?'待機中':'走行中',delayMs:train.delay_ms};
 if(state.state==='moving'){
  const arrived=now>=state.arrival_ms,remaining=state.release_ms-now;
  // This is the current leg's trailing-clearance event, not a promise
  // that every reservation is released (through/terminal leases can remain).
  const detail=arrived&&Number.isSafeInteger(state.release_ms)&&remaining>0
   ?`この区間の後部通過まで ${Math.ceil(remaining/1000)}秒`:undefined;
  return {status:arrived?'到着済み':'走行中',detail,delayMs:train.delay_ms};
 }
 if(now<state.ready_ms)return {status:train.leg>0?'停車中':'発車待ち',delayMs:train.delay_ms};
 // The native clock has already attempted dispatch at ready_ms. The engine
 // books the wait on entry, so accrue it in the view only while waiting.
 const reason=train.waiting_for==='crossing'?'平面交差':train.waiting_for==='junction'?'分岐・接続':train.waiting_for==='platform'?'ホームにいる別の列車':train.waiting_for==='track'?'線路':null;
 const detail=train.waiting_for==='junction'
  ?train.wait_scope==='held_across_connections'?'相手列車が、接続部をまたぐ進路を確保中'
   :train.wait_scope==='needs_across_connections'?'次の停車駅まで、接続部をまたぐ進路が空くのを待っています':undefined
  :undefined;
 const waited=now-state.ready_ms;
 return {status:reason?`進路待ち（${reason}）`:'進路待ち',detail,
  currentWait:reason&&Number.isSafeInteger(waited)&&waited>0?`今回の進路待ち ${Math.ceil(waited/1000)}秒`:undefined,
  delayMs:train.delay_ms+waited};
}

function ActiveVehicles({trains,configured,now,invalidated,bus=false}:{trains:Train[];configured:boolean;now:number;invalidated:boolean;bus?:boolean}){
 const label=bus?"バス":"列車";
 return <div className="operation-trains">
  {trains.length?<ul aria-label={`運行中・待機中の${label}`}>{trains.map(train=>{
   const {status,detail,currentWait,delayMs}=vehicleStatus(train,now,invalidated,bus);
   return <li key={train.key.scheduled_ms}><span>{scheduleTime(train.key.scheduled_ms)} 発</span><strong>{status}</strong>{detail&&<small>{detail}</small>}{currentWait&&<small>{currentWait}</small>}{delayMs>0&&<span>遅れ {Math.ceil(delayMs/1000)}秒</span>}</li>;
  })}</ul>:<p>{configured?`現在、運行中・待機中の${label}はありません。`:'この系統のダイヤは未設定です。'}</p>}
 </div>;
}

export function BusOperations({routes,enabled}:{routes:{id:string;name:string;pattern?:string|null}[];enabled:boolean}){
 const state=useContext(Context),[routeId,setRouteId]=useState('');
 if(!state||!state.view||!routes.length)return null;
 const {view,busy,blocked,send,error}=state;
 const route=routes.find(row=>row.id===routeId)??routes[0];
 const config=view.bus_services?.find(row=>row.route_id===route.id);
 const blocker=view.timetable_edit_blocker;
 const trains=view.trains.filter(train=>train.key.service_id===`bus:${route.id}`);
 const disabled=!enabled||busy||blocked;
 return <section className="service-operations" aria-label="バスの運行">
  <h4>バスの運行</h4>
  <label>運行するバス系統<select value={route.id} disabled={disabled} onChange={e=>setRouteId(e.target.value)}>{routes.map(row=><option key={row.id} value={row.id}>{row.name}</option>)}</select></label>
  <TimetableForm key={route.id+':'+JSON.stringify(config??null)} bus ordinaryRoundTrip={route.pattern==='openRoundTrip'} serviceId={route.id} config={config} now={view.now_ms} disabled={disabled||!!blocker} send={send} expectedOperationToken={view.timetable_edit_token}/>
  {blocker?<p>{blocker}</p>:view.now_ms>0&&<p>現在の時刻と他のダイヤはそのままです。変更後の過去の便は走りません。</p>}
  <ActiveVehicles trains={trains} configured={!!config} now={view.now_ms} invalidated={view.invalidated} bus/>
  {error&&<p role="status">時刻欄の運行エラーを確認してください。</p>}
 </section>;
}
