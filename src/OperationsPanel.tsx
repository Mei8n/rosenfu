import {createContext,useContext,useEffect,useRef,useState,type FormEvent,type ReactNode} from 'react';
import {invoke} from '@tauri-apps/api/core';
import './operationsPanel.css';

type Timetable={first_departure_ms:number;last_departure_ms:number;headway_ms:number};
type Vehicle={max_speed_kmh:number;acceleration_mps2:number;braking_mps2:number;length_m:number};
type ServiceConfig={service_id:string;timetable:Timetable;vehicle:Vehicle;dwell_ms:number};
type Train={key:{service_id:string;scheduled_ms:number};leg:number;delay_ms:number;state:{state:'waiting';ready_ms:number}|{state:'moving';entered_ms:number;arrival_ms:number;release_ms:number}};
export type OperationView={now_ms:number;paused:boolean;speed:number;services:ServiceConfig[];trains:Train[]};
type Commands={set_operation_clock:{paused:boolean;speed:number};configure_operation_service:{serviceId:string;timetable:Timetable;vehicle:Vehicle;dwellMs:number}};
type OperationContext={view:OperationView|null;busy:boolean;blocked:boolean;error:string;refresh:()=>Promise<void>;send:<K extends keyof Commands>(command:K,args:Commands[K])=>Promise<void>};
const Context=createContext<OperationContext|null>(null);
const speeds=[1,2,4,8,16] as const;
const minute=60_000,day=24*60*minute;

export function OperationProvider({available,blocked,epoch,onChange,children}:{available:boolean;blocked:boolean;epoch:number;onChange:()=>void;children:ReactNode}){
 const [snapshot,setSnapshot]=useState<{epoch:number;view:OperationView}|null>(null);
 const [busy,setBusy]=useState(false),[error,setError]=useState('');
 const generation=useRef(0),flight=useRef<{done:Promise<void>}|null>(null),intent=useRef<object|null>(null);
 const gate=useRef({available,blocked,epoch,onChange});gate.current={available,blocked,epoch,onChange};
 const view=snapshot?.epoch===epoch?snapshot.view:null;
 async function request(command:string,args?:Commands[keyof Commands],tick=false){
  const before=gate.current,token=generation.current;
  if(!before.available||before.blocked||(tick&&(flight.current||intent.current))||(!tick&&intent.current))return;
  const current=()=>token===generation.current&&before.epoch===gate.current.epoch&&gate.current.available;
  const userIntent={};
  if(!tick){intent.current=userIntent;setBusy(true);setError('');}
  let lock:{done:Promise<void>}|undefined;
  let release:()=>void=()=>{};
  try{
   // An explicit operation waits for a pending timer tick instead of being lost.
   if(flight.current)await flight.current.done;
   if(!current()||gate.current.blocked)return;
   lock={done:new Promise<void>(resolve=>{release=resolve;})};flight.current=lock;
   const next=await invoke<OperationView>(command,args);
   if(!current())return;
   if(!Number.isSafeInteger(next?.now_ms)||next.now_ms<0||typeof next.paused!=='boolean'||!speeds.some(speed=>speed===next.speed)||!Array.isArray(next.services)||!Array.isArray(next.trains))throw Error('運行エンジンの応答を確認できませんでした。');
   setSnapshot({epoch:before.epoch,view:next});
   if(!tick&&command!=='operation_snapshot')gate.current.onChange();
  }catch(cause){
   if(current())setError(String(cause));
  }finally{
   if(flight.current===lock)flight.current=null;release();
   if(intent.current===userIntent){intent.current=null;setBusy(false);}
  }
 }
 // A world's reply may never replace a newly loaded world's clock.
 useEffect(()=>{
  generation.current++;flight.current=null;intent.current=null;setBusy(false);setError('');
  return()=>{generation.current++;flight.current=null;intent.current=null;};
 },[available,epoch]);
 useEffect(()=>{if(available&&!blocked&&!view&&!error)void request('operation_snapshot');},[available,blocked,epoch,view,error]);
 const timerEnabled=available&&!blocked&&!!view&&!view.paused&&!error;
 const tick=useRef(()=>request('advance_operation',undefined,true));
 tick.current=()=>request('advance_operation',undefined,true);
 useEffect(()=>{
  if(!timerEnabled)return;
  // Only the native monotonic clock decides elapsed game time.
  const id=window.setInterval(()=>{if(!flight.current&&!intent.current)void tick.current();},500);
  return()=>window.clearInterval(id);
 },[timerEnabled,epoch]);
 const state:OperationContext|null=available?{view,busy,blocked,error,refresh:()=>request('operation_snapshot'),send:request}:null;
 return <Context.Provider value={state}>{children}</Context.Provider>;
}

function scheduleTime(ms:number){
 const within=ms%day,hours=Math.floor(within/(60*minute)),minutes=Math.floor(within/minute)%60;
 return `${Math.floor(ms/day)+1}日目 ${String(hours).padStart(2,'0')}:${String(minutes).padStart(2,'0')}`;
}

export function OperationClock(){
 const state=useContext(Context);if(!state)return null;
 const {view,busy,blocked,error,refresh,send}=state;
 const disabled=busy||blocked;
 return <div className="operation-clock" role="group" aria-label="ゲーム内時刻">
  {view?<>
   <output aria-live="off">{scheduleTime(view.now_ms)}</output>
   <button type="button" className="secondary" disabled={disabled} onClick={()=>void send('set_operation_clock',{paused:!view.paused,speed:view.speed})}>{view.paused?'再開':'一時停止'}</button>
   <label>速度<select value={view.speed} disabled={disabled} onChange={e=>void send('set_operation_clock',{paused:view.paused,speed:Number(e.target.value)})}>{speeds.map(speed=><option key={speed} value={speed}>{speed}倍</option>)}</select></label>
  </>:<span>{error?'時刻を取得できません':'時刻を確認中…'}</span>}
  {error&&<><span role="alert">{error}</span><button type="button" className="secondary" disabled={disabled} onClick={()=>void refresh()}>運行状態を再取得</button></>}
 </div>;
}

type Form={firstDay:string;firstTime:string;lastDay:string;lastTime:string;headway:string;dwell:string;maxSpeed:string;acceleration:string;length:string};
function departureFields(ms:number){
 const within=ms%day,h=Math.floor(within/(60*minute)),m=Math.floor(within/minute)%60,s=Math.floor(within/1000)%60;
 return {day:String(Math.floor(ms/day)+1),time:[h,m,s].map(v=>String(v).padStart(2,'0')).join(':')};
}
function initialForm(config:ServiceConfig|undefined,now:number):Form{
 const first=departureFields(config?.timetable.first_departure_ms??Math.ceil(now/minute)*minute);
 const last=departureFields(config?.timetable.last_departure_ms??Math.ceil(now/minute)*minute+18*60*minute);
 return {firstDay:first.day,firstTime:first.time,lastDay:last.day,lastTime:last.time,headway:String((config?.timetable.headway_ms??10*minute)/minute),dwell:String((config?.dwell_ms??30_000)/1000),maxSpeed:String(config?.vehicle.max_speed_kmh??100),acceleration:String(config?.vehicle.acceleration_mps2??0.8),length:String(config?.vehicle.length_m??80)};
}
function departure(dayValue:string,time:string){
 const d=Number(dayValue),parts=time.split(':').map(Number);
 if(!Number.isSafeInteger(d)||d<1||!/^\d{2}:\d{2}(?::\d{2})?$/.test(time)||parts[0]>23||parts[1]>59||(parts[2]??0)>59) return NaN;
 return (d-1)*day+parts[0]*60*minute+parts[1]*minute+(parts[2]??0)*1000;
}

function TimetableForm({serviceId,config,now,disabled,send}:{serviceId:string;config?:ServiceConfig;now:number;disabled:boolean;send:OperationContext['send']}){
 const [form,setForm]=useState(()=>initialForm(config,now));
 const [validation,setValidation]=useState('');
 const update=(key:keyof Form,value:string)=>{setForm(f=>({...f,[key]:value}));setValidation('');};
 function save(event:FormEvent<HTMLFormElement>){
  event.preventDefault();if(disabled)return;
  const start=departure(form.firstDay,form.firstTime),end=departure(form.lastDay,form.lastTime);
  const timetable={first_departure_ms:start,last_departure_ms:end,headway_ms:Math.round(Number(form.headway)*minute)};
  const vehicle={max_speed_kmh:Number(form.maxSpeed),acceleration_mps2:Number(form.acceleration),braking_mps2:config?.vehicle.braking_mps2??1,length_m:Number(form.length)};
  const dwellMs=Math.round(Number(form.dwell)*1000);
  if(![start,end,timetable.headway_ms,dwellMs].every(Number.isSafeInteger)||end<start||timetable.headway_ms<=0||dwellMs<0||!Object.values(vehicle).every(n=>Number.isFinite(n)&&n>0)||!form.dwell.trim()){
   setValidation('発車時刻・運転間隔・車両性能を確認してください。終発は初発以降にしてください。');return;
  }
  void send('configure_operation_service',{serviceId,timetable,vehicle,dwellMs});
 }
 const number=(key:keyof Form,label:string,min:number,step:string|number='any')=><label>{label}<input type="number" required min={min} step={step} value={form[key]} onChange={e=>update(key,e.target.value)}/></label>;
 return <form className="operation-form" onSubmit={save}><fieldset disabled={disabled}><legend>簡易ダイヤ</legend>
  <div className="operation-times">{number('firstDay','初発の日（何日目）',1,1)}<label>初発時刻<input type="time" required step="1" value={form.firstTime} onChange={e=>update('firstTime',e.target.value)}/></label>
  {number('lastDay','終発の日（何日目）',1,1)}<label>終発時刻<input type="time" required step="1" value={form.lastTime} onChange={e=>update('lastTime',e.target.value)}/></label></div>
  <div className="operation-times">{number('headway','運転間隔（分）',0.01)}{number('dwell','駅の停車時間（秒）',0)}</div>
  <details><summary>使用車両の性能</summary><div className="operation-times">{number('maxSpeed','最高速度（km/h）',1)}{number('acceleration','加速度（m/s²）',0.01)}{number('length','編成長（m）',1)}</div></details>
  {validation&&<p role="alert">{validation}</p>}
  <button type="submit">{config?'ダイヤの変更を適用':'運行を設定'}</button>
 </fieldset></form>;
}

export function ServiceOperations({serviceId,enabled}:{serviceId:string;enabled:boolean}){
 const state=useContext(Context);if(!state||!state.view)return null;
 const {view,busy,blocked,send,error}=state;
 const config=view.services.find(row=>row.service_id===serviceId);
 const trains=view.trains.filter(train=>train.key.service_id===serviceId);
 return <section className="service-operations" aria-label="系統の運行">
  <h4>運行</h4>
  <TimetableForm key={serviceId+':'+JSON.stringify(config??null)} serviceId={serviceId} config={config} now={view.now_ms} disabled={!enabled||busy||blocked} send={send}/>
  <div className="operation-trains">
   {trains.length?<ul aria-label="運行中・待機中の列車">{trains.map(train=><li key={train.key.scheduled_ms}><span>{scheduleTime(train.key.scheduled_ms)} 発</span><strong>{train.state.state==='waiting'?'待機中':'走行中'}</strong>{train.delay_ms>0&&<span>遅れ {Math.ceil(train.delay_ms/1000)}秒</span>}</li>)}</ul>:<p>{config?'現在、運行中・待機中の列車はありません。':'この系統のダイヤは未設定です。'}</p>}
  </div>
  {error&&<p role="status">時刻欄の運行エラーを確認してください。</p>}
 </section>;
}
