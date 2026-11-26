type Summary={newCrossingCount:number;roadsUnverified:boolean};
function summary(value:unknown):Summary|null{
 if(!value||typeof value!=='object')return null;
 if(!('newCrossingCount' in value)||!('roadsUnverified' in value))return null;
 const {newCrossingCount,roadsUnverified}=value;
 if(typeof newCrossingCount!=='number'||!Number.isSafeInteger(newCrossingCount)||newCrossingCount<0||typeof roadsUnverified!=='boolean')return null;
 return {newCrossingCount,roadsUnverified};
}
/** A reference estimate only. Economy/charging is not implemented. Never turn
 * pending, failed or incomplete road data into a definitive zero crossings. */
export function CrossingEstimate({value,pending=false}:{value:unknown;pending?:boolean}){
 if(value&&typeof value==="object"&&"validationError" in value&&typeof value.validationError==="string")return null;
 const data=summary(value);
 if(pending||!data)return <div className="crossing-estimate">踏切：確認中</div>;
 return <div className="crossing-estimate">
  <div>踏切 {data.newCrossingCount}か所{data.roadsUnverified?'（確認済み分）':''}</div>
  <div>追加額（参考）{data.newCrossingCount}億円</div>
  <small>{data.roadsUnverified?'道路に未確認部分あり。':''}1か所1億円・課金未実装</small>
 </div>;
}
