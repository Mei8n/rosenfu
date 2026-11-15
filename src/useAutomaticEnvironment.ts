import {useEffect,useMemo,useState} from 'react';
import {createRoadRepository,roadBoundsKey,type RoadDataState,type RoadSnapshot} from './automaticRoadData';
import {createWaterRepository} from './automaticWaterData';
import {corridorBounds,combineCorridor} from './environmentCorridor';
import type {GraphBounds,GraphPoint} from './referenceGraph';
import type {EnvironmentSnapshot} from './environmentData';
let roads:ReturnType<typeof createRoadRepository>|undefined,water:ReturnType<typeof createWaterRepository>|undefined;
/** Stop scheduling obsolete work on pan/plan/world changes. In-flight bounded downloads
 * may populate the cache, but only the current generation can publish a complete corridor. */
export function useAutomaticEnvironment(bounds:GraphBounds,enabled:boolean,worldIdentity:unknown,saved?:EnvironmentSnapshot,lines:readonly (readonly GraphPoint[])[]=[]){
 const view={west:Math.floor(bounds.west*200)/200,south:Math.floor(bounds.south*200)/200,east:Math.ceil(bounds.east*200)/200,north:Math.ceil(bounds.north*200)/200};
 const key=JSON.stringify({view,lines});
 const request=useMemo(()=>JSON.parse(key) as {view:GraphBounds;lines:GraphPoint[][]},[key]);
 const [state,setState]=useState<{key:string;world:unknown;road:RoadDataState;water:{status:string;message:string};data?:EnvironmentSnapshot}>();
 const [attempt,setAttempt]=useState(0);
 useEffect(()=>{let active=true;let regions:GraphBounds[];
  if(!enabled)return;
  const publish=(road:RoadDataState,wet:{status:string;message:string},data?:EnvironmentSnapshot)=>{if(active)setState({key,world:worldIdentity,road,water:wet,data});};
  try{regions=corridorBounds(request.lines);if(!regions.length){roadBoundsKey(request.view);regions=[request.view];}}
  catch(error){publish({status:'unavailable',message:String(error)},{status:'unavailable',message:'水域データ未取得'});return;}
  const timer=setTimeout(()=>{void (async()=>{
   roads??=createRoadRepository();water??=createWaterRepository();
   const parts:Parameters<typeof combineCorridor>[0]=[];let first:RoadSnapshot|undefined;
   try{
    for(const region of regions){
     if(!active)return;
     publish({status:'loading',message:`道路データ取得中 ${parts.length}/${regions.length}`},{status:'loading',message:'水域データ取得中'});
     const road=await roads.load(region);if(!active)return;first??=road;
     const wet=await water.load(region);if(!active)return;
     parts.push({road,water:wet});
     // Bound aggregation while collecting, not only after all network requests.
     combineCorridor(parts);
    }
    const data=combineCorridor(parts);
    publish({status:'ready',snapshot:first,message:`道路取得済み：${data.roads.length} way（OSM）`},{status:'ready',message:'水域取得済み（国土地理院）'},data);
   }catch(error){publish({status:'error',message:`道路・水域データ未取得：${String(error)}`},{status:'error',message:'計画回廊の取得が未完了です。'});}
  })();},500);
  return ()=>{active=false;clearTimeout(timer);};
 },[key,request,enabled,worldIdentity,attempt]);
 const current=enabled&&state?.key===key&&state.world===worldIdentity?state:undefined;
 return {road:current?.road??{status:'unavailable' as const,message:'道路データ未取得'},water:current?.water??{status:'unavailable',message:'水域データ未取得'},data:current?.data??saved??null,retry:()=>setAttempt(a=>a+1)};
}
