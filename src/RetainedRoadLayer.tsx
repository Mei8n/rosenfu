import {invoke} from '@tauri-apps/api/core';
import {useEffect,useMemo,useRef,useState} from 'react';
import {DEFAULT_CAMERA,MAP_VIEWBOX,project,type Camera} from './map';
import type {GraphBounds} from './referenceGraph';

type CellRect=readonly [number,number,number,number];
type Point=readonly [longitude:number,latitude:number];
type Segment=readonly [Point,Point];
type RetainedView={requested_tiles:number;retained_tiles:number;segments:readonly Segment[];truncated:boolean};

const EDGE=2**14, MAX_CELLS=16, DEBOUNCE_MS=180;
export function retainedRoadRect({west,north,east,south}:GraphBounds):CellRect|null{
 const x=(longitude:number)=>(longitude+180)/360*EDGE;
 const y=(latitude:number)=>(1-Math.asinh(Math.tan(latitude*Math.PI/180))/Math.PI)/2*EDGE;
 if(![west,north,east,south].every(Number.isFinite)||west>=east||north<=south||west< -180||east>180||north>85.05112878||south< -85.05112878)return null;
 const cell=(value:number)=>Math.max(0,Math.min(EDGE-1,value));
 const rect=[cell(Math.floor(x(west))),cell(Math.floor(y(north))),cell(Math.ceil(x(east))-1),cell(Math.ceil(y(south))-1)] as const;
 const columns=rect[2]-rect[0]+1, rows=rect[3]-rect[1]+1;
 return columns>0&&rows>0&&columns*rows<=MAX_CELLS?rect:null;
}

type Query={key:string;epoch:number;basis:string};
type Published=Query&({value:RetainedView}|{error:true});

/** The view never supplies route proof or changes the ordinary online reference. */
export function useRetainedRoadView(enabled:boolean,bounds:GraphBounds,epoch:number,basis:string){
 const rect=enabled?retainedRoadRect(bounds):null;
 const key=rect?.join(',')??'';
 const [published,setPublished]=useState<Published>();
 const inFlight=useRef<Promise<void>|null>(null);
 useEffect(()=>{
  if(!enabled||!key)return;
  let active=true;
  const timer=setTimeout(()=>{
   void (async()=>{
    // At most one worker at a time. A viewport replaced while waiting doesn't
    // start a worker, and a previous response cannot reveal old-world roads.
    if(inFlight.current)await inFlight.current;
    if(!active)return;
    const query={key,epoch,basis};
    const work=invoke<RetainedView>('retained_road_view',{rect:key.split(',').map(Number)}).then(
     value=>{if(active)setPublished({...query,value});},
     ()=>{if(active)setPublished({...query,error:true});},
    );
    inFlight.current=work;
    await work;
    if(inFlight.current===work)inFlight.current=null;
   })();
  },DEBOUNCE_MS);
  return()=>{active=false;clearTimeout(timer);};
 },[enabled,key,epoch,basis]);
 const current=enabled&&key&&published?.key===key&&published.epoch===epoch&&published.basis===basis?published:undefined;
 const value=current&&'value' in current?current.value:undefined;
 const message=!enabled?'':!rect?'道路を表示するには地図を拡大':current&&'error' in current?'ワールド内の参考道路を表示できません':!value?'表示中の範囲を確認中':value.truncated?'見えている道路の一部を表示中。拡大すると絞れます':'保持している道路データ：'+value.retained_tiles+'/'+value.requested_tiles+'区画';
 return {value,message};
}

const BASE={...DEFAULT_CAMERA,zoom:14};
export function RetainedRoadLayer({view,camera}:{view:RetainedView|undefined;camera:Camera}){
 // The view and both cameras use Web Mercator. Pan and zoom are affine,
 // so camera frames can move the existing path without rebuilding up to
 // 4,000 segments on every frame.
 const d=useMemo(()=>view?.segments.map(([a,b])=>{
  const from=project(a[1],a[0],BASE),to=project(b[1],b[0],BASE);
  return 'M'+from.x.toFixed(4)+','+from.y.toFixed(4)+'L'+to.x.toFixed(4)+','+to.y.toFixed(4);
 }).join(' '),[view]);
 if(!d)return null;
 const scale=2**(camera.zoom-BASE.zoom);
 const center=project(BASE.latitude,BASE.longitude,camera);
 const transform='translate('+(center.x-MAP_VIEWBOX.width/2*scale)+' '+(center.y-MAP_VIEWBOX.height/2*scale)+') scale('+scale+')';
 return <path data-retained-road-reference d={d} transform={transform} fill="none" stroke="#ad6c24" strokeWidth="1.5" strokeOpacity=".7" strokeDasharray="4 2" vectorEffect="non-scaling-stroke" pointerEvents="none"/>;
}
