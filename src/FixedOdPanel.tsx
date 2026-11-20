import {useEffect,useRef,useState,type FormEvent} from 'react';
import {invoke} from '@tauri-apps/api/core';
import './fixedOdPanel.css';
type Named={id:string;name:string};
export type FixedOdScenario={origin_station_id:string;destination_station_id:string;trips:number|null;alternative_seconds:number|null};
export type FixedOdScenarioView={token:string;scenario:FixedOdScenario|null};
type Allocation={stations:string[];services:string[];generalized_seconds:number;trips:number;time?:{walk_seconds:number;wait_seconds:number;ride_seconds:number;transfers:number}};
type Result={potential_trips:number|null;rail:Allocation[];alternative_trips:number;unserved_trips:number;unresolved_trips:number;search_complete:boolean};
const quantity=(n:number)=>new Intl.NumberFormat('ja-JP',{maximumFractionDigits:1}).format(n);

export function FixedOdPanel({stations,services,busStops=[],busRoutes=[],enabled,worldVersion,saveScenario}:{stations:Named[];services:Named[];busStops?:Named[];busRoutes?:Named[];enabled:boolean;worldVersion:unknown;saveScenario?:(expectedToken:string,scenario:FixedOdScenario|null)=>Promise<FixedOdScenarioView>}){
 const [origin,setOrigin]=useState(''),[destination,setDestination]=useState('');
 const [trips,setTrips]=useState('100'),[alternative,setAlternative]=useState('');
 const [settled,setSettled]=useState<{world:unknown;result:Result}|null>(null);
 const [failure,setFailure]=useState<{world:unknown;message:string}|null>(null);
 const [savedView,setSavedView]=useState<FixedOdScenarioView>();
 const saved=savedView?.scenario;
 const [savePending,setSavePending]=useState(false),[saveError,setSaveError]=useState('');
 const scenarioFlight=useRef(false);
 const persistence=Boolean(saveScenario);
 useEffect(()=>{
  if(!persistence)return;
  let active=true;
  void invoke<FixedOdScenarioView>('fixed_od_scenario').then(view=>{
   if(!active)return;
   if(typeof view?.token!=='string')throw Error('保存済み需要条件の応答を確認できませんでした。');
   setSavedView(view);
   const {scenario}=view;
   if(!scenario)return;
   setOrigin(scenario.origin_station_id);setDestination(scenario.destination_station_id);
   setTrips(scenario.trips===null?'':String(scenario.trips));
   setAlternative(scenario.alternative_seconds===null?'':String(scenario.alternative_seconds/60));
  }).catch(cause=>{if(active)setSaveError(String(cause));});
  return ()=>{active=false;};
 },[persistence]);
 const [pending,setPending]=useState(false);
 const latest=useRef({worldVersion,enabled}),flight=useRef<object|null>(null);
 latest.current={worldVersion,enabled};
 const result=settled?.world===worldVersion?settled?.result:null;
 const error=failure?.world===worldVersion?failure?.message:null;
 const stops=busStops.map(stop=>({...stop,id:`bus:${stop.id}`}));
 const stationNames=new Map([...stations,...stops].map(s=>[s.id,s.name]));
 const serviceNames=new Map(services.map(s=>[s.id,s.name]));
 for(const route of busRoutes)serviceNames.set(`bus:${route.id}`,route.name);
 const dirty=()=>{setSettled(null);setFailure(null);};
 function stationSelector(label:string,value:string,onChange:(value:string)=>void){
  return <label>{label}<select required value={value} onChange={event=>{onChange(event.target.value);dirty();}}>
   <option value="">{stops.length?'地点を選択':'駅を選択'}</option>
   {stations.length>0&&<optgroup label="鉄道の駅">{stations.map(station=><option key={station.id} value={station.id}>{station.name}</option>)}</optgroup>}
   {stops.length>0&&<optgroup label="バス停">{stops.map(stop=><option key={stop.id} value={stop.id}>{stop.name}</option>)}</optgroup>}
  </select></label>;
 }
 const endpointsSelected=stationNames.has(origin)&&stationNames.has(destination)&&origin!==destination;
 const savedOrphan=Boolean(saved&&(!stationNames.has(saved.origin_station_id)||!stationNames.has(saved.destination_station_id)));
 function input():FixedOdScenario|null{
  if(!endpointsSelected)return null;
  const count=trips.trim()?Number(trips):null,other=alternative.trim()?Math.round(Number(alternative)*60):null;
  if(count!==null&&(!Number.isSafeInteger(count)||count<0||count>1_000_000_000)||other!==null&&(!Number.isSafeInteger(other)||other<=0||other>31_536_000))return null;
  return {origin_station_id:origin,destination_station_id:destination,trips:count,alternative_seconds:other};
 }
 async function storeScenario(clear:boolean){
  if(!enabled||!saveScenario||!savedView||scenarioFlight.current)return;
  const next=clear?null:input();
  if(!clear&&!next){setSaveError('人数は0〜10億の整数、代替手段の所要時間は正の値かつ1年以下を入力してください。');return;}
  scenarioFlight.current=true;setSavePending(true);setSaveError('');
  try{setSavedView(await saveScenario(savedView.token,next));}
  catch(cause){setSaveError(String(cause));}
  finally{scenarioFlight.current=false;setSavePending(false);}
 }
 async function calculate(event:FormEvent<HTMLFormElement>){
  event.preventDefault();
  if(!enabled||flight.current||scenarioFlight.current||!endpointsSelected)return;
  const scenario=input();
  if(!scenario){setFailure({world:worldVersion,message:'人数は0〜10億の整数、代替手段の所要時間は正の値かつ1年以下を入力してください。'});return;}
  const captured=worldVersion,lock={};flight.current=lock;setPending(true);dirty();
  try{
   const response=await invoke<Result>('preview_fixed_od',{originStationId:origin,destinationStationId:destination,trips:scenario.trips,alternativeSeconds:scenario.alternative_seconds});
   if(latest.current.worldVersion!==captured||!latest.current.enabled)return;
   if(typeof response?.search_complete!=='boolean'||!Array.isArray(response.rail))throw Error('需要計算の応答を確認できませんでした。');
   setSettled({world:captured,result:response});
  }catch(cause){
   if(latest.current.worldVersion===captured&&latest.current.enabled)setFailure({world:captured,message:String(cause)});
  }finally{if(flight.current===lock){flight.current=null;setPending(false);}}
 }
 return <section className="fixed-od-panel" aria-label="固定ODの需要試算">
  <h4>固定ODの需要試算</h4>
  <p>指定した{stops.length?"2地点":"2駅"}間の仮の移動需要を、現在のダイヤで試算します。実人口・実際の利用者数ではありません。</p>
  {stops.length>0&&<p>鉄道とバスの乗り継ぎはまだ計算しません。</p>}
  {persistence&&savedOrphan&&<p role="status">保存済みの出発地または到着地がなくなりました。条件を選び直してください。</p>}
  {stationNames.size<2?<p>出発地と到着地を作成してください。</p>:<form onSubmit={event=>void calculate(event)}>
   <fieldset disabled={!enabled||pending||savePending||persistence&&saved===undefined&&!saveError}>
    {stationSelector(stops.length?'出発地点':'出発駅',origin,setOrigin)}{stationSelector(stops.length?'到着地点':'到着駅',destination,setDestination)}
    <label>移動需要（人）<input type="number" min="0" max="1000000000" placeholder="未入力は不明" step="1" value={trips} onChange={event=>{setTrips(event.target.value);dirty();}}/></label>
    <label>鉄道以外の所要時間（分・任意）<input type="number" min="0.1" step="any" value={alternative} onChange={event=>{setAlternative(event.target.value);dirty();}}/></label>
    <button type="submit" disabled={!endpointsSelected}>需要を試算</button>
    {persistence&&<button type="button" disabled={!endpointsSelected||saved===undefined} onClick={()=>void storeScenario(false)}>この試算条件をワールドに記録</button>}
   </fieldset>
  </form>}
  {persistence&&saved&&<><p>記録済みの条件は次のセーブに含まれます。</p><button type="button" disabled={!enabled||pending||savePending} onClick={()=>void storeScenario(true)}>記録した条件を外す</button></>}
  {saveError&&<p role="alert">{saveError}</p>}
  {pending&&<p role="status">需要を試算中…</p>}
  {error&&<p role="alert">{error}</p>}
  {result&&(result.potential_trips===null?<p role="status">移動需要は不明のため、人数は配分していません。</p>:result.search_complete?<dl className="fixed-od-results">
   <dt>公共交通を利用</dt><dd>{quantity(result.rail.reduce((sum,path)=>sum+path.trips,0))}人</dd>
   <dt>別の移動手段を利用</dt><dd>{quantity(result.alternative_trips)}人</dd>
   <dt>利用できる経路なし</dt><dd>{quantity(result.unserved_trips)}人</dd>
   <dt>未確定</dt><dd>{quantity(result.unresolved_trips)}人</dd>
  </dl>:<p role="status">探索上限に達しました。{quantity(result.unresolved_trips)}人は未確定で、利用人数には含めません。</p>)}
  {result?.search_complete&&result.rail.length>0&&<ol className="fixed-od-paths" aria-label="利用経路">
   {result.rail.map((path,index)=><li key={index}><strong>{quantity(path.trips)}人</strong> {path.stations.map(id=>stationNames.get(id)??'移動地点').join(' → ')}{path.services.length>0&&<div>{path.services.map(id=>serviceNames.get(id)??'別系統').join(' → ')}</div>}{path.time&&<div>推計: 待ち {quantity(path.time.wait_seconds/60)}分 / 乗車 {quantity(path.time.ride_seconds/60)}分 / 徒歩 {quantity(path.time.walk_seconds/60)}分 / 乗換 {path.time.transfers}回</div>}</li>)}
  </ol>}
 </section>;
}
