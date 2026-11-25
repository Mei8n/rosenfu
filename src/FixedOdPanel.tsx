import {useEffect,useRef,useState,type FormEvent} from 'react';
import {invoke} from '@tauri-apps/api/core';
import './fixedOdPanel.css';
import {OperationObservationMinute} from './OperationsPanel';
type Named={id:string;name:string};
export type FixedOdScenario={origin_station_id:string;destination_station_id:string;trips:number|null;alternative_seconds:number|null};
export type FixedOdScenarioView={token:string;scenario:FixedOdScenario|null};
type Allocation={stations:string[];services:string[];generalized_seconds:number;trips:number;time?:{walk_seconds:number;wait_seconds:number;ride_seconds:number;transfers:number}};
type Result={potential_trips:number|null;rail:Allocation[];alternative_trips:number;unserved_trips:number;unresolved_trips:number;search_complete:boolean};
type ChoicePrices={publicFareYen:number;alternativeCostYen:number;valueOfTimeYenPerHour:number};
type BudgetInput={prices:ChoicePrices;marginalYen:number};
type ObservedResult={
 status:'reached'|'incomplete'|'no_arrival_within_window'|'outside_budget';
 potential_trips:number|null;rail_trips:number;alternative_trips:number;unresolved_trips:number;
 itinerary?:{ready_ms:number;arrival_ms:number|null;platform_wait_ms:number;onboard_ms:number}|null;
};
const quantity=(n:number)=>new Intl.NumberFormat('ja-JP',{maximumFractionDigits:1}).format(n);
const yen=(amount:number)=>`${quantity(Math.round(amount))}円`;
const yenInput=(value:string,minimum=0)=>{
 const amount=value.trim()?Number(value):NaN;
 return Number.isSafeInteger(amount)&&amount>=minimum&&amount<=1_000_000?amount:null;
};
function BudgetResult({paths,budget,names}:{paths:Allocation[];budget:BudgetInput;names:Map<string,string>}){
 const counts=new Map<string,number>();
 for(const path of paths)for(const service of path.services)counts.set(service,(counts.get(service)??0)+path.trips);
 const rows=[...counts.entries()].sort((a,b)=>b[1]-a[1]||a[0].localeCompare(b[0]));
 const boardings=rows.reduce((sum,[,count])=>sum+count,0);
 const fares=boardings*budget.prices.publicFareYen,cost=boardings*budget.marginalYen;
 return <section aria-label="この需要条件の運賃と費用の概算">
  <h5>この需要条件の運賃と費用の概算</h5>
  <p>試算した延べ乗車回数に入力単価を掛けた比較です。実際の売上・利益ではなく、建設費・固定費・ほかの区間の利用を含みません。</p>
  <dl className="fixed-od-results">
   <dt>延べ乗車</dt><dd>{quantity(boardings)}回</dd>
   <dt>運賃収入の概算</dt><dd>{yen(fares)}</dd>
   <dt>乗車ごとにかかる費用の概算</dt><dd>{yen(cost)}</dd>
   <dt>運賃と入力費用との差額</dt><dd>{yen(fares-cost)}</dd>
  </dl>
  {rows.length>0&&<ul aria-label="系統別の延べ乗車と概算差額">{rows.map(([id,n])=><li key={id}>
   {names.get(id)??'別系統'}: {quantity(n)}回 / {yen(n*(budget.prices.publicFareYen-budget.marginalYen))}
  </li>)}</ul>}
 </section>;
}

export function FixedOdPanel({stations,services,busStops=[],busRoutes=[],enabled,worldVersion,saveScenario}:{stations:Named[];services:Named[];busStops?:Named[];busRoutes?:Named[];enabled:boolean;worldVersion:unknown;saveScenario?:(expectedToken:string,scenario:FixedOdScenario|null)=>Promise<FixedOdScenarioView>}){
 const [origin,setOrigin]=useState(''),[destination,setDestination]=useState('');
 const [useTransfer,setUseTransfer]=useState(false);
 const [transferStation,setTransferStation]=useState(''),[transferStop,setTransferStop]=useState(''),[walkMinutes,setWalkMinutes]=useState('');
 const [trips,setTrips]=useState('100'),[alternative,setAlternative]=useState('');
 const [useBudget,setUseBudget]=useState(false);
 const [fareYen,setFareYen]=useState(''),[otherYen,setOtherYen]=useState('0');
 const [hourYen,setHourYen]=useState(''),[marginalYen,setMarginalYen]=useState('0');
 const [settled,setSettled]=useState<{world:unknown;result:Result;budget:BudgetInput|null}|null>(null);
 const [settledObserved,setSettledObserved]=useState<{world:unknown;result:ObservedResult}|null>(null);
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
 const currentResult=settled?.world===worldVersion?settled:null;
 const result=currentResult?.result;
 const observed=settledObserved?.world===worldVersion?settledObserved?.result:null;
 const observationStart=observed?.itinerary?.ready_ms;
 const error=failure?.world===worldVersion?failure?.message:null;
 const stops=busStops.map(stop=>({...stop,id:`bus:${stop.id}`}));
 const stationNames=new Map([...stations,...stops].map(s=>[s.id,s.name]));
 const serviceNames=new Map(services.map(s=>[s.id,s.name]));
 for(const route of busRoutes)serviceNames.set(`bus:${route.id}`,route.name);
 const dirty=()=>{setSettled(null);setSettledObserved(null);setFailure(null);};
 function stationSelector(label:string,value:string,onChange:(value:string)=>void){
  return <label>{label}<select required value={value} onChange={event=>{onChange(event.target.value);dirty();}}>
   <option value="">{stops.length?'地点を選択':'駅を選択'}</option>
   {stations.length>0&&<optgroup label="鉄道の駅">{stations.map(station=><option key={station.id} value={station.id}>{station.name}</option>)}</optgroup>}
   {stops.length>0&&<optgroup label="バス停">{stops.map(stop=><option key={stop.id} value={stop.id}>{stop.name}</option>)}</optgroup>}
  </select></label>;
 }
 const endpointsSelected=stationNames.has(origin)&&stationNames.has(destination)&&origin!==destination;
 const observedAllowed=endpointsSelected&&!origin.startsWith('bus:')&&!destination.startsWith('bus:')&&!useTransfer&&!useBudget;
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
  const minutes=Number(walkMinutes);
  const transfer=useTransfer?{stationId:transferStation,busStopId:transferStop,walkSeconds:minutes*60}:null;
  if(transfer&&(!stations.some(s=>s.id===transferStation)||!stops.some(s=>s.id===transferStop)||!Number.isSafeInteger(minutes)||minutes<1||minutes>60)){
   setSettled(null);
   setFailure({world:worldVersion,message:'徒歩乗り換えには駅・バス停と1〜60分の所要時間を指定してください。'});
   return;
  }
  let budget:BudgetInput|null=null;
  if(useBudget){
   const publicFareYen=yenInput(fareYen),alternativeCostYen=scenario.alternative_seconds===null?0:yenInput(otherYen);
   const valueOfTimeYenPerHour=yenInput(hourYen,1),marginal=yenInput(marginalYen);
   if(publicFareYen===null||alternativeCostYen===null||valueOfTimeYenPerHour===null||marginal===null){
    setSettled(null);
    setFailure({world:worldVersion,message:'運賃・費用は0〜100万円、時間価値は1〜100万円/時の整数を入力してください。'});
    return;
   }
   budget={prices:{publicFareYen,alternativeCostYen,valueOfTimeYenPerHour},marginalYen:marginal};
  }
  const captured=worldVersion,lock={};flight.current=lock;setPending(true);setFailure(null);setSettled(null);
  try{
   const response=await invoke<Result>('preview_fixed_od',{originStationId:origin,destinationStationId:destination,trips:scenario.trips,alternativeSeconds:scenario.alternative_seconds,...(transfer?{transfer}:{}),...(budget?{prices:budget.prices}:{})});
   if(latest.current.worldVersion!==captured||!latest.current.enabled)return;
   if(typeof response?.search_complete!=='boolean'||!Array.isArray(response.rail))throw Error('需要計算の応答を確認できませんでした。');
   setSettled({world:captured,result:response,budget});
  }catch(cause){
   if(latest.current.worldVersion===captured&&latest.current.enabled)setFailure({world:captured,message:String(cause)});
  }finally{if(flight.current===lock){flight.current=null;setPending(false);}}
 }
 async function calculateObserved(){
  if(!enabled||flight.current||scenarioFlight.current||!observedAllowed)return;
  const scenario=input();
  if(!scenario){setFailure({world:worldVersion,message:'人数は0〜10億の整数、代替手段の所要時間は正の値かつ1年以下を入力してください。'});return;}
  const captured=worldVersion,lock={};flight.current=lock;setPending(true);setFailure(null);setSettledObserved(null);
  try{
   const response=await invoke<ObservedResult>('preview_observed_fixed_od',{query:{
    originStationId:scenario.origin_station_id,destinationStationId:scenario.destination_station_id,
    trips:scenario.trips,alternativeSeconds:scenario.alternative_seconds
   }});
   if(latest.current.worldVersion!==captured||!latest.current.enabled)return;
   if(!response||!['reached','incomplete','no_arrival_within_window','outside_budget'].includes(response.status))throw Error('遅延を含む試算の応答を確認できませんでした。');
   setSettledObserved({world:captured,result:response});
  }catch(cause){
   if(latest.current.worldVersion===captured&&latest.current.enabled)setFailure({world:captured,message:String(cause)});
  }finally{if(flight.current===lock){flight.current=null;setPending(false);}}
 }
 return <section className="fixed-od-panel" aria-label="固定ODの需要試算">
  <h4>固定ODの需要試算</h4>
  <p>指定した{stops.length?"2地点":"2駅"}間の仮の移動需要を、現在のダイヤで試算します。実人口・実際の利用者数ではありません。</p>
  {stops.length>0&&<p>駅とバス停の徒歩乗り換えは、試算で指定した場合だけ計算します。</p>}
  {persistence&&savedOrphan&&<p role="status">保存済みの出発地または到着地がなくなりました。条件を選び直してください。</p>}
  {stationNames.size<2?<p>出発地と到着地を作成してください。</p>:<form onSubmit={event=>void calculate(event)}>
   <fieldset disabled={!enabled||pending||savePending||persistence&&saved===undefined&&!saveError}>
    {stationSelector(stops.length?'出発地点':'出発駅',origin,setOrigin)}{stationSelector(stops.length?'到着地点':'到着駅',destination,setDestination)}
    <label>移動需要（人）<input type="number" min="0" max="1000000000" placeholder="未入力は不明" step="1" value={trips} onChange={event=>{setTrips(event.target.value);dirty();}}/></label>
    <label>{stops.length?"公共交通以外":"鉄道以外"}の所要時間（分・任意）<input type="number" min="0.1" step="any" value={alternative} onChange={event=>{setAlternative(event.target.value);dirty();}}/></label>
    {stops.length>0&&stations.length>0&&<fieldset>
     <legend>試算用の徒歩乗り換え</legend>
     <label className="fixed-od-transfer-toggle"><input type="checkbox" checked={useTransfer} onChange={event=>{setUseTransfer(event.target.checked);dirty();}}/>駅とバス停の乗り換えを試す</label>
     {useTransfer&&<>
      <label>乗り換え駅<select value={transferStation} onChange={event=>{setTransferStation(event.target.value);dirty();}}>
       <option value="">駅を選択</option>{stations.map(station=><option key={station.id} value={station.id}>{station.name}</option>)}
      </select></label>
      <label>乗り換えバス停<select value={transferStop} onChange={event=>{setTransferStop(event.target.value);dirty();}}>
       <option value="">バス停を選択</option>{stops.map(stop=><option key={stop.id} value={stop.id}>{stop.name}</option>)}
      </select></label>
      <label>徒歩の所要時間（分）<input type="number" min="1" max="60" step="1" value={walkMinutes} onChange={event=>{setWalkMinutes(event.target.value);dirty();}}/></label>
      <p>徒歩は両方向に同じ時間で試算します。実際の徒歩経路や設備ではなく、この条件は保存しません。</p>
     </>}
    </fieldset>}
    <fieldset>
     <legend>試算用の運賃・費用</legend>
     <label className="fixed-od-transfer-toggle"><input type="checkbox" checked={useBudget} onChange={event=>{setUseBudget(event.target.checked);dirty();}}/>運賃と費用を比較する</label>
     {useBudget&&<>
      <label>公共交通の運賃（円／乗車）<input type="number" required min="0" max="1000000" step="1" value={fareYen} onChange={event=>{setFareYen(event.target.value);dirty();}}/></label>
      {alternative.trim()&&<label>代替手段の費用（円／移動）<input type="number" required min="0" max="1000000" step="1" value={otherYen} onChange={event=>{setOtherYen(event.target.value);dirty();}}/></label>}
      <label>時間価値（円／時）<input type="number" required min="1" max="1000000" step="1" value={hourYen} onChange={event=>{setHourYen(event.target.value);dirty();}}/></label>
      <label>乗車ごとにかかる費用（円／乗車）<input type="number" required min="0" max="1000000" step="1" value={marginalYen} onChange={event=>{setMarginalYen(event.target.value);dirty();}}/></label>
      <p>運賃は同じ車両に乗り続ける間は1回分、乗り換えるともう1回分です。時間価値で費用を時間に換算して経路選択を比較します。費用条件は保存せず、遅れを含む試算では使えません。</p>
     </>}
    </fieldset>
    <button type="submit" disabled={!endpointsSelected}>需要を試算</button>
    <button type="button" disabled={!observedAllowed} onClick={()=>void calculateObserved()}>運行の遅れを含めて試算</button>
    <p>遅れを含む試算は、今のゲーム時刻に出発駅へ着いた人を対象に、鉄道だけを30分間再現します。試算方式と結果は保存しません。</p>
    {persistence&&<button type="button" disabled={!endpointsSelected||saved===undefined} onClick={()=>void storeScenario(false)}>この試算条件をワールドに記録</button>}
   </fieldset>
  </form>}
  {persistence&&saved&&<><p>記録済みの条件は次のセーブに含まれます。</p><button type="button" disabled={!enabled||pending||savePending} onClick={()=>void storeScenario(true)}>記録した条件を外す</button></>}
  {saveError&&<p role="alert">{saveError}</p>}
  {pending&&<p role="status">需要を試算中…</p>}
  {error&&<p role="alert">{error}</p>}
  {result&&observed&&<p>同じ条件での別々の試算です。人数の差は遅れだけによるものではありません。</p>}
  {result&&<section aria-label="ダイヤを使った需要試算結果">
   <h5>ダイヤを使った需要試算結果</h5>
   {result.potential_trips===null?<p role="status">移動需要は不明のため、人数は配分していません。</p>:result.search_complete?<dl className="fixed-od-results">
    <dt>公共交通を利用</dt><dd>{quantity(result.rail.reduce((sum,path)=>sum+path.trips,0))}人</dd>
    <dt>別の移動手段を利用</dt><dd>{quantity(result.alternative_trips)}人</dd>
    <dt>利用できる経路なし</dt><dd>{quantity(result.unserved_trips)}人</dd>
    <dt>未確定</dt><dd>{quantity(result.unresolved_trips)}人</dd>
   </dl>:<p role="status">探索上限に達しました。{quantity(result.unresolved_trips)}人は未確定で、利用人数には含めません。</p>}
   {result.search_complete&&result.rail.length>0&&<ol className="fixed-od-paths" aria-label="利用経路">
    {result.rail.map((path,index)=><li key={index}><strong>{quantity(path.trips)}人</strong> {path.stations.map(id=>stationNames.get(id)??'移動地点').join(' → ')}{path.services.length>0&&<div>{path.services.map(id=>serviceNames.get(id)??'別系統').join(' → ')}</div>}{path.time&&<div>推計: 待ち {quantity(path.time.wait_seconds/60)}分 / 乗車 {quantity(path.time.ride_seconds/60)}分 / 徒歩 {quantity(path.time.walk_seconds/60)}分 / 乗換 {path.time.transfers}回</div>}</li>)}
   </ol>}
   {currentResult?.budget&&result.search_complete&&result.potential_trips!==null&&<BudgetResult paths={result.rail} budget={currentResult.budget} names={serviceNames}/>}
  </section>}
  {observed&&<section aria-label="遅れを含む需要試算結果">
   <h5>遅れを含む需要試算結果</h5>
   {typeof observationStart==='number'&&Number.isSafeInteger(observationStart)&&observationStart>=0&&<OperationObservationMinute observedMs={observationStart}/>}
    <p>観測開始時点に出発駅に着いた場合の最早到着1経路と、指定した代替手段を比較します。ダイヤ全体の利用予測ではありません。</p>
   {observed.status!=='reached'?<p role="status">
    {observed.status==='no_arrival_within_window'?'30分以内に鉄道で到着できる経路を確認できませんでした。':'運行の再現が試算上限に達しました。'}
    {observed.potential_trips===null?'需要人数は不明で、配分していません。':`移動需要 ${quantity(observed.unresolved_trips)}人は未確定です。`}
   </p>:<>
    {observed.potential_trips===null?<p role="status">移動需要は不明のため、人数は配分していません。</p>:<dl className="fixed-od-results">
     <dt>この鉄道経路を利用</dt><dd>{quantity(observed.rail_trips)}人</dd>
     <dt>別の移動手段を利用</dt><dd>{quantity(observed.alternative_trips)}人</dd>
    </dl>}
    {observed.itinerary?.arrival_ms!=null&&<p>所要 {quantity((observed.itinerary.arrival_ms-observed.itinerary.ready_ms)/60000)}分（待ち {quantity(observed.itinerary.platform_wait_ms/60000)}分、乗車 {quantity(observed.itinerary.onboard_ms/60000)}分）</p>}
   </>}
  </section>}
 </section>;
}
