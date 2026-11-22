import type {GeoPoint} from './alignment';
import {project,type Camera} from './map';
import {restoreOrdinaryBusControls,type BusInteriorStop,type OrdinaryBusPath,type OrdinaryBusPreview,type OrdinaryBusPreviewReply} from './ordinaryBusDraft';

export type StopOccurrenceChoice={segment:number;occurrenceKey:string;fractionQ:number;distance:number};
const keyPattern=/^[0-9a-f]{64}$/;
const idPattern=/^[A-Za-z0-9_-]{1,80}$/;
const STOP_DENOMINATOR=1_000_000;
const MAX_STOPS=64;
export function acceptableBusStopName(value:string){
 const name=value.trim();
 return !!name&&!/\p{Cc}/u.test(name)&&new TextEncoder().encode(name).length<=200;
}

export function verifyBusStopPlacement(path:OrdinaryBusPath,reply:OrdinaryBusPreviewReply){
 const {preview}=restoreOrdinaryBusControls(path,reply);
 const keys=preview.stopOccurrenceKeys,data=path.interiorStops??[],views=preview.interiorStops??[];
 const invalid=()=>Error('停留所の位置を確認できません。バス経路を選び直してください。');
 if(!keys||!Array.isArray(keys)||keys.length<1||keys.length>4096||keys.length!==preview.points.length-1||
   keys.some(key=>typeof key!=='string'||!keyPattern.test(key))||new Set(keys).size!==keys.length||preview.returnUnavailableReason||
   data.length>MAX_STOPS||(path.pattern==='closedOneWay'&&!!preview.loopFallbackStop!==(data.length===0))||!Array.isArray(views)||views.length!==data.length||
   new Set(views.map(view=>view.id)).size!==views.length||new Set(views.map(view=>view.stopId)).size!==views.length)throw invalid();
 const index=new Map(keys.map((key,i)=>[key,i]));
 const positions=new Set<string>(),ids=new Set<string>();
 for(const stop of data){
  const {id,name,occurrenceKey,fractionQ}=stop,position=occurrenceKey+':'+fractionQ,i=index.get(occurrenceKey);
  if(!idPattern.test(id)||ids.has(id)||positions.has(position)||i===undefined||!acceptableBusStopName(name)||
     !Number.isInteger(fractionQ)||fractionQ<1||fractionQ>=STOP_DENOMINATOR)throw invalid();
  ids.add(id);positions.add(position);
  const view=views.find(view=>view.id===id);
  if(!view||view.name!==name||typeof view.stopId!=='string'||!view.stopId||
     typeof view.servesReturn!=='boolean'||view.servesReturn&&!preview.returnPath)throw invalid();
  const [a,b]=[preview.points[i],preview.points[i+1]],r=fractionQ/STOP_DENOMINATOR;
  if(!view.point||!Number.isFinite(view.point.latitude)||!Number.isFinite(view.point.longitude)||
     Math.abs(view.point.latitude-(a.latitude+(b.latitude-a.latitude)*r))>1e-8||
     Math.abs(view.point.longitude-(a.longitude+(b.longitude-a.longitude)*r))>1e-8)throw invalid();
 }
 return preview as OrdinaryBusPreview & {stopOccurrenceKeys:string[]};
}

/** Return every near-equally close traversal, not just a road vertex. A
 * repeated traversal with identical drawing still carries a distinct key. */
export function pickBusStopOccurrences(preview:OrdinaryBusPreview & {stopOccurrenceKeys:string[]},at:GeoPoint,camera:Camera,scale:number):StopOccurrenceChoice[]{
 if(!Number.isFinite(scale)||scale<=0)return [];
 const target=project(at.latitude,at.longitude,camera);
 const candidates:StopOccurrenceChoice[]=[];
 let a=project(preview.points[0].latitude,preview.points[0].longitude,camera);
 for(let i=0;i<preview.stopOccurrenceKeys.length;i++){
  const next=preview.points[i+1],b=project(next.latitude,next.longitude,camera);
  const dx=b.x-a.x,dy=b.y-a.y,square=dx*dx+dy*dy;
  if(square*scale*scale>=1){
   const t=((target.x-a.x)*dx+(target.y-a.y)*dy)/square;
   // Never clamp a hit at the node to a pretend interior location.
   const fractionQ=Math.round(t*STOP_DENOMINATOR);
   if(fractionQ>0&&fractionQ<STOP_DENOMINATOR){
    const distance=Math.hypot(target.x-a.x-t*dx,target.y-a.y-t*dy)*scale;
    if(distance<=14)candidates.push({segment:i,occurrenceKey:preview.stopOccurrenceKeys[i],fractionQ,distance});
   }
  }
  a=b;
 }
 if(!candidates.length)return [];
 const least=Math.min(...candidates.map(c=>c.distance));
 return candidates.filter(c=>c.distance<=least+0.75);
}


/** Suggestions are hints from the worker, never a replacement for authored
 * stops or a reason to skip final native verification before save. */
export function appendBusStopSuggestions(path:OrdinaryBusPath,preview:OrdinaryBusPreview & {stopOccurrenceKeys:string[]},suggestions:unknown):OrdinaryBusPath{
 if(!Array.isArray(suggestions))throw Error('停留所の自動配置を確認できません。');
 const baseline=path.interiorStops??[];
 if(baseline.length+suggestions.length>MAX_STOPS)throw Error('停留所が多すぎます。');
 const authorized=new Set(preview.stopOccurrenceKeys);
 const ids=new Set(baseline.map(st=>st.id)),positions=new Set(baseline.map(st=>st.occurrenceKey+':'+st.fractionQ));
 const added:BusInteriorStop[]=[];
 for(const s of suggestions){
  if(!s||typeof s!=='object')throw Error('停留所の自動配置を確認できません。');
  const {id,name,occurrenceKey,fractionQ}=s as Record<string,unknown>;
  if(typeof id!=='string'||!idPattern.test(id)||ids.has(id)||typeof name!=='string'||!acceptableBusStopName(name)||
    typeof occurrenceKey!=='string'||!authorized.has(occurrenceKey)||typeof fractionQ!=='number'||!Number.isInteger(fractionQ)||
    fractionQ<1||fractionQ>=STOP_DENOMINATOR||positions.has(occurrenceKey+':'+fractionQ))throw Error('停留所の自動配置を確認できません。');
  ids.add(id);positions.add(occurrenceKey+':'+fractionQ);added.push({id,name,occurrenceKey,fractionQ});
 }
 return added.length?{...path,interiorStops:[...baseline,...added]}:path;
}
