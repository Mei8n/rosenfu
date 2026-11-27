import {useState,type FormEvent} from 'react';

const MAX_DISTANCE_METRES=10_000_000;
const MAX_FARE_YEN=1_000_000;
const MAX_BANDS=12;
type FareBand={maxMetres:string;fareYen:string};
type Quote={status:'quoted';fareYen:number;maxMetres:number}|{status:'unverified'|'distance'|'table'|'outside'};
const integer=(text:string,min:number,max:number)=>{
 const raw=text.trim();
 const n=/^\d{1,8}$/.test(raw)?Number(raw):NaN;
 return Number.isSafeInteger(n)&&n>=min&&n<=max?n:null;
};

/** An explicitly entered, single fare-area what-if, not a world fare or an OD allocation. */
export function quoteDistanceFare(sameArea:boolean,shortestMetresText:string,bands:readonly FareBand[]):Quote {
 if(!sameArea)return {status:'unverified'};
 const metres=integer(shortestMetresText,1,MAX_DISTANCE_METRES);
 if(metres===null)return {status:'distance'};
 if(bands.length===0||bands.length>MAX_BANDS)return {status:'table'};
 let previous=0,match:Extract<Quote,{status:'quoted'}>|undefined;
 for(const row of bands){
  const limit=integer(row.maxMetres,1,MAX_DISTANCE_METRES);
  const fare=integer(row.fareYen,0,MAX_FARE_YEN);
  if(limit===null||fare===null||limit<=previous)return {status:'table'};
  if(!match&&metres<=limit)match={status:'quoted',maxMetres:limit,fareYen:fare};
  previous=limit;
 }
 return match??{status:'outside'};
}

const errors={
 unverified:'出発地から到着地まで同じ運賃圏に収まることを確認してください。',
 distance:'確認済みの最短営業キロを、1〜10,000,000の整数メートルで入力してください。',
 table:'上限距離を小さい順に重複なく並べ、各運賃を0〜100万円の整数で入力してください。',
 outside:'この距離を含む運賃区分がありません。最後の上限を超える運賃は推定しません。',
};
const number=new Intl.NumberFormat('ja-JP');
const blankBand=()=>({maxMetres:'',fareYen:''});

export function DistanceFareQuote(){
 const [sameArea,setSameArea]=useState(false);
 const [metres,setMetres]=useState('');
 const [bands,setBands]=useState<FareBand[]>(()=>[blankBand()]);
 const [quote,setQuote]=useState<Quote|null>(null);
 function updateBand(index:number,key:keyof FareBand,value:string){
  setBands(current=>current.map((band,i)=>i===index?{...band,[key]:value}:band));setQuote(null);
 }
 function calculate(event:FormEvent<HTMLFormElement>){
  event.preventDefault();
  setQuote(quoteDistanceFare(sameArea,metres,bands));
 }
 return <details>
  <summary>距離帯運賃の手入力試算</summary>
  <p>別枠の運賃見積もりです。地図や線路から距離・運賃圏は推定しません。自分で確認した最短営業キロと運賃表を入れてください。需要試算・収支・セーブには反映しません。</p>
  <form noValidate onSubmit={calculate}>
   <fieldset>
    <legend>同一運賃圏の普通運賃</legend>
    <label className="fixed-od-transfer-toggle"><input type="checkbox" checked={sameArea}
     onChange={e=>{setSameArea(e.target.checked);setQuote(null);}}/>出発地から到着地まで同じ運賃圏と確認済み</label>
    <label>確認済みの最短営業キロ（m）<input type="text" inputMode="numeric" maxLength={8} value={metres}
     onChange={e=>{setMetres(e.target.value);setQuote(null);}}/></label>
    <p>上限距離ちょうどはその区分です。同じ運賃圏内での乗り換えは、別の乗車券として加算しません。別運賃圏への乗り換えや加算料金は対象外です。</p>
    {bands.map((band,i)=><div key={i}>
     <label>区分 {i+1} の上限（m）<input type="text" inputMode="numeric" maxLength={8} value={band.maxMetres}
      onChange={e=>updateBand(i,'maxMetres',e.target.value)}/></label>
     <label>区分 {i+1} の運賃（円）<input type="text" inputMode="numeric" maxLength={7} value={band.fareYen}
      onChange={e=>updateBand(i,'fareYen',e.target.value)}/></label>
     {i===bands.length-1&&i>0&&<button type="button" onClick={()=>{setBands(current=>current.slice(0,-1));setQuote(null);}}>最後の区分を削除</button>}
    </div>)}
    <button type="button" disabled={bands.length>=MAX_BANDS}
     onClick={()=>{setBands(current=>[...current,blankBand()]);setQuote(null);}}>区分を追加</button>
    <button type="submit">距離帯運賃を見積もる</button>
    {quote&&(quote.status==='quoted'
     ?<p role="status">入力した運賃表では {number.format(quote.fareYen)}円（{number.format(quote.maxMetres)} mまで）</p>
     :<p role="alert">{errors[quote.status]}</p>)}
   </fieldset>
  </form>
 </details>;
}
