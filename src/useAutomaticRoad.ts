import {useEffect,useState} from 'react';
import {createRoadProvider,type RoadProvider} from './roadProvider';
import {roadBoundsKey,type RoadDataState} from './automaticRoadData';
import type {GraphBounds} from './referenceGraph';
let provider:RoadProvider|undefined;
/** Road display/following is independent of water availability. No construction
 * approval or crossing assessment is implied by a successfully loaded reference. */
export function useAutomaticRoad(bounds:GraphBounds,enabled:boolean,world:unknown,frozen=false){
 const view={west:Math.floor(bounds.west*200)/200,south:Math.floor(bounds.south*200)/200,east:Math.ceil(bounds.east*200)/200,north:Math.ceil(bounds.north*200)/200};
 const key=JSON.stringify(view);
 const [result,setResult]=useState<{key:string;world:unknown;state:RoadDataState}>();
 const [attempt,setAttempt]=useState(0);
 useEffect(()=>{
  if(!enabled||frozen)return;let active=true;
  const publish=(state:RoadDataState)=>{if(active)setResult({key,world,state});};
  const area=JSON.parse(key) as GraphBounds;
  try{roadBoundsKey(area);}catch(error){publish({status:'unavailable',message:String(error)});return;}
  publish({status:'loading',message:'道路データ取得中'});
  const timer=setTimeout(()=>{void(async()=>{try{
   provider??=createRoadProvider();const snapshot=await provider.load(area);
   publish({status:'ready',snapshot,message:`道路参考データ取得済み（${provider.name}）。現実の全道路の網羅性は保証しません。`});
  }catch(error){publish({status:'error',message:`道路参考データ未取得：${String(error)}`});}})();},300);
  return()=>{active=false;clearTimeout(timer);};
 },[key,enabled,world,frozen,attempt]);
 const state=enabled&&result&&result.world===world&&(result.key===key||frozen)?result.state:{status:'unavailable' as const,message:'道路参考データ未取得'};
 return {...state,retry:()=>setAttempt(n=>n+1)};
}
