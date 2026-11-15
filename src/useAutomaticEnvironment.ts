import {useEffect,useMemo,useState} from 'react';
import {createRoadRepository,roadBoundsKey,type RoadDataState} from './automaticRoadData';
import {createWaterRepository} from './automaticWaterData';
import type {GraphBounds} from './referenceGraph';
import type {EnvironmentSnapshot} from './environmentData';
let roads:ReturnType<typeof createRoadRepository>|undefined,water:ReturnType<typeof createWaterRepository>|undefined;
/** Bounds keys and effect cleanup reject late responses after pan/load/unmount.
 * Shared downloads continue for cache reuse, but can never publish into an obsolete view. */
export function useAutomaticEnvironment(bounds:GraphBounds,enabled:boolean,worldIdentity:unknown,saved?:EnvironmentSnapshot){
 const key=JSON.stringify({west:Math.floor(bounds.west*200)/200,south:Math.floor(bounds.south*200)/200,east:Math.ceil(bounds.east*200)/200,north:Math.ceil(bounds.north*200)/200});
 const region=useMemo(()=>JSON.parse(key) as GraphBounds,[key]);
 const [road,setRoad]=useState<RoadDataState>({status:'unavailable',message:'道路データ未取得'});
 const [wet,setWet]=useState<{status:string;message:string;coverage?:GraphBounds[];water?:EnvironmentSnapshot['water']}>({status:'unavailable',message:'水域データ未取得'});
 const [attempt,setAttempt]=useState(0);
 const [loadedKey,setLoadedKey]=useState('');const [loadedWorld,setLoadedWorld]=useState<unknown>();
 useEffect(()=>{let active=true;setLoadedKey('');setRoad({status:'unavailable',message:'道路データ未取得'});setWet({status:'unavailable',message:'水域データ未取得'});
  if(!enabled)return;
  try{roadBoundsKey(region);}catch(error){setRoad({status:'unavailable',message:String(error)});return;}
  const timer=setTimeout(()=>{
   setRoad({status:'loading',message:'道路データ取得中'});setWet({status:'loading',message:'水域データ取得中'});
   roads??=createRoadRepository();water??=createWaterRepository();
   void roads.load(region).then(snapshot=>{if(active){setRoad({status:'ready',snapshot,message:`道路取得済み：${snapshot.roads.length} way（OSM）`});setLoadedKey(key);setLoadedWorld(worldIdentity);}},error=>{if(active)setRoad({status:'error',message:`道路データ未取得：${String(error)}`});});
   void water.load(region).then(result=>{if(active)setWet({status:'ready',message:'水域取得済み（国土地理院）',...result});},error=>{if(active)setWet({status:'error',message:`水域データ未取得：${String(error)}`});});
  },500);
  return ()=>{active=false;clearTimeout(timer);};
 },[key,enabled,worldIdentity,attempt,region]);
 // Do not expose an old ready snapshot during the render before effect cleanup.
 const current=enabled&&loadedKey===key&&loadedWorld===worldIdentity;
 const data=useMemo<EnvironmentSnapshot|null>(()=>current&&road.snapshot?{revision:road.snapshot.bundle.graphSha256,road_coverage:[road.snapshot.bounds],roads:road.snapshot.roads,water_coverage:wet.coverage??[],water:wet.water??[]}:saved??null,[current,road.snapshot,wet,saved]);
 return {road:current?road:{...road,snapshot:undefined},water:wet,data,retry:()=>setAttempt(a=>a+1)};
}
