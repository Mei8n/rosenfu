import {useLayoutEffect,useRef} from 'react';
import {MAP_VIEWBOX,project,type Camera,type mapViewport} from './map';

type GeoPoint={latitude:number;longitude:number};
type MarkerKey={service_id:string;scheduled_ms:number};
type Marker={key:MarkerKey;entered_ms:number;alignment_start:GeoPoint;alignment_end:GeoPoint;edge_start:GeoPoint;edge_end:GeoPoint;position:number};
const MAX_MARKERS=32;
function isGeoPoint(value:unknown):value is GeoPoint{
 if(!value||typeof value!=='object')return false;
 const {latitude,longitude}=value as Partial<GeoPoint>;
 return typeof latitude==='number'&&Number.isFinite(latitude)&&Math.abs(latitude)<=85.05112878
  &&typeof longitude==='number'&&Number.isFinite(longitude)&&Math.abs(longitude)<=180;
}
function markerPosition(value:unknown,mode:'rail'|'bus'){
 if(!value||typeof value!=='object')return null;
 const {key,entered_ms,alignment_start,alignment_end,edge_start,edge_end,position}=value as Partial<Marker>;
 const start=mode==='bus'?edge_start:alignment_start;
 const end=mode==='bus'?edge_end:alignment_end;
 if(!key||typeof key.service_id!=='string'||(mode==='bus'?!key.service_id.startsWith('bus:')||key.service_id.length<=4||key.service_id.length>132:!/^[1-9][0-9]{0,19}$/.test(key.service_id))||
  !Number.isSafeInteger(key.scheduled_ms)||key.scheduled_ms<0||
  !isGeoPoint(start)||!isGeoPoint(end)||typeof position!=='number'||!Number.isFinite(position)||position<0||position>1||
  (start.latitude===end.latitude&&start.longitude===end.longitude))return null;
 return {key,start,end,position,enteredMs:Number.isSafeInteger(entered_ms)&&typeof entered_ms==='number'&&entered_ms>=0?entered_ms:undefined};
}

type MarkerPoint={x:number;y:number;key:string;motion?:string};
function OperationDot({point,mode,radius,clockMs,animate,camera}:{
 point:MarkerPoint;mode:'rail'|'bus';radius:number;clockMs?:number;animate:boolean;camera:Camera;
}){
 const circle=useRef<SVGCircleElement|null>(null);
 const previous=useRef<{point:MarkerPoint;clockMs?:number;camera:Camera;radius:number;animate:boolean}|null>(null);
 const {x,y,motion}=point;
 const {latitude,longitude,zoom}=camera;
 useLayoutEffect(()=>{
  const before=previous.current;
  previous.current={point:{...point},clockMs,camera:{latitude,longitude,zoom},radius,animate};
  const node=circle.current;
  if(!animate||!before?.animate||!motion||before.point.motion!==motion||!node?.animate||
   clockMs===undefined||before.clockMs===undefined||clockMs<=before.clockMs||
   before.camera.latitude!==latitude||before.camera.longitude!==longitude||before.camera.zoom!==zoom||
   before.radius!==radius||window.matchMedia?.('(prefers-reduced-motion: reduce)').matches)return;
  const dx=before.point.x-x,dy=before.point.y-y;
  if(dx===0&&dy===0)return;
  // Both endpoints are committed samples on the same painted chord. A brief
  // visual blend stays between them; it never predicts position past the last
  // update, bends across a junction, or advances the simulation clock.
  const animation=node.animate(
   [{transform:`translate(${dx}px,${dy}px)`},{transform:'translate(0px,0px)'}],
   {duration:180,easing:'linear'},
  );
  return ()=>animation.cancel();
 },[x,y,motion,clockMs,animate,latitude,longitude,zoom,radius]);
 return <circle ref={circle} cx={x} cy={y} r={radius} fill={mode==='bus'?'#007383':'#1453a0'} stroke="white" strokeWidth="1.5" vectorEffect="non-scaling-stroke"/>;
}

// Passive, committed samples only. No map hit targets, independent polling,
// prediction or raw engine identities in DOM.
export function OperationMapMarkers({markers,camera,scale,crop={x:0,y:0,...MAP_VIEWBOX,scale},mode='rail',clockMs,animate=false}:{markers:unknown;camera:Camera;scale:number;crop?:ReturnType<typeof mapViewport>;mode?:'rail'|'bus';clockMs?:number;animate?:boolean}){
 if(!Array.isArray(markers)||markers.length===0||markers.length>MAX_MARKERS||!(Number.isFinite(scale)&&scale>0))return null;
 const keys=new Set<string>();
 const radius=4/scale;
 const relevant=markers.flatMap(value=>{
  const marker=markerPosition(value,mode);if(!marker)return [];
  const a=project(marker.start.latitude,marker.start.longitude,camera);
  const b=project(marker.end.latitude,marker.end.longitude,camera);
  // Paint uses a straight segment between projected alignment endpoints.
  // Interpolate along that same chord using native time/distance, not lat/lon.
  const x=a.x+(b.x-a.x)*marker.position,y=a.y+(b.y-a.y)*marker.position;
  if(x<crop.x-radius||y<crop.y-radius||x>crop.x+crop.width+radius||y>crop.y+crop.height+radius)return [];
  const key=marker.key.service_id+':'+marker.key.scheduled_ms;
  if(keys.has(key))return [];
  keys.add(key);
  const motion=marker.enteredMs===undefined?undefined:JSON.stringify([marker.enteredMs,marker.start.latitude,marker.start.longitude,marker.end.latitude,marker.end.longitude]);
  return [{x,y,key,motion}];
 });
 if(!relevant.length)return null;
 return <g pointerEvents="none" aria-label={mode==='bus'?'表示できる走行中のバス':'表示できる走行中の列車'}>
  {relevant.map(point=><OperationDot key={point.key} point={point} mode={mode} radius={radius} clockMs={clockMs} animate={animate} camera={camera}/>)}
 </g>;
}
