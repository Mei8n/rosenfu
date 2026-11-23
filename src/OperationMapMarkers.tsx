import {MAP_VIEWBOX,project,type Camera} from './map';

type GeoPoint={latitude:number;longitude:number};
type Marker={key:{service_id:string;scheduled_ms:number};alignment_start:GeoPoint;alignment_end:GeoPoint;position:number};
const MAX_MARKERS=32;
function isGeoPoint(value:unknown):value is GeoPoint{
 if(!value||typeof value!=='object')return false;
 const {latitude,longitude}=value as Partial<GeoPoint>;
 return typeof latitude==='number'&&Number.isFinite(latitude)&&Math.abs(latitude)<=85.05112878
  &&typeof longitude==='number'&&Number.isFinite(longitude)&&Math.abs(longitude)<=180;
}
function isMarker(value:unknown):value is Marker{
 if(!value||typeof value!=='object')return false;
 const {key,alignment_start,alignment_end,position}=value as Partial<Marker>;
 return !!key&&typeof key.service_id==='string'&&/^[1-9][0-9]{0,19}$/.test(key.service_id)
  &&Number.isSafeInteger(key.scheduled_ms)&&key.scheduled_ms>=0
  &&isGeoPoint(alignment_start)&&isGeoPoint(alignment_end)
  &&typeof position==='number'&&Number.isFinite(position)&&position>=0&&position<=1
  &&(alignment_start.latitude!==alignment_end.latitude||alignment_start.longitude!==alignment_end.longitude);
}

// Passive, committed positions only. No interpolation, map hit targets,
// timers, polling, or raw engine identities in DOM.
export function OperationMapMarkers({markers,camera,scale}:{markers:unknown;camera:Camera;scale:number}){
 if(!Array.isArray(markers)||markers.length===0||markers.length>MAX_MARKERS||!(Number.isFinite(scale)&&scale>0))return null;
 const keys=new Set<string>();
 const radius=4/scale;
 const relevant=markers.flatMap(marker=>{
  if(!isMarker(marker))return [];
  const a=project(marker.alignment_start.latitude,marker.alignment_start.longitude,camera);
  const b=project(marker.alignment_end.latitude,marker.alignment_end.longitude,camera);
  // Paint uses a straight segment between projected alignment endpoints.
  // Interpolate along that same chord using native time/distance, not lat/lon.
  const x=a.x+(b.x-a.x)*marker.position,y=a.y+(b.y-a.y)*marker.position;
  if(x < -radius||y < -radius||x>MAP_VIEWBOX.width+radius||y>MAP_VIEWBOX.height+radius)return [];
  const key=marker.key.service_id+':'+marker.key.scheduled_ms;
  if(keys.has(key))return [];
  keys.add(key);
  return [{x,y,key}];
 });
 if(!relevant.length)return null;
 return <g pointerEvents="none" aria-label="表示できる走行中の列車">
  {relevant.map(({x,y,key})=><circle key={key} cx={x} cy={y} r={radius} fill="#1453a0" stroke="white" strokeWidth="1.5" vectorEffect="non-scaling-stroke"/>)}
 </g>;
}
