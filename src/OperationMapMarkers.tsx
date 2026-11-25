import {MAP_VIEWBOX,project,type Camera,type mapViewport} from './map';

type GeoPoint={latitude:number;longitude:number};
type MarkerKey={service_id:string;scheduled_ms:number};
type Marker={key:MarkerKey;alignment_start:GeoPoint;alignment_end:GeoPoint;edge_start:GeoPoint;edge_end:GeoPoint;position:number};
const MAX_MARKERS=32;
function isGeoPoint(value:unknown):value is GeoPoint{
 if(!value||typeof value!=='object')return false;
 const {latitude,longitude}=value as Partial<GeoPoint>;
 return typeof latitude==='number'&&Number.isFinite(latitude)&&Math.abs(latitude)<=85.05112878
  &&typeof longitude==='number'&&Number.isFinite(longitude)&&Math.abs(longitude)<=180;
}
function markerPosition(value:unknown,mode:'rail'|'bus'){
 if(!value||typeof value!=='object')return null;
 const {key,alignment_start,alignment_end,edge_start,edge_end,position}=value as Partial<Marker>;
 const start=mode==='bus'?edge_start:alignment_start;
 const end=mode==='bus'?edge_end:alignment_end;
 if(!key||typeof key.service_id!=='string'||(mode==='bus'?!key.service_id.startsWith('bus:')||key.service_id.length<=4||key.service_id.length>132:!/^[1-9][0-9]{0,19}$/.test(key.service_id))||
  !Number.isSafeInteger(key.scheduled_ms)||key.scheduled_ms<0||
  !isGeoPoint(start)||!isGeoPoint(end)||typeof position!=='number'||!Number.isFinite(position)||position<0||position>1||
  (start.latitude===end.latitude&&start.longitude===end.longitude))return null;
 return {key,start,end,position};
}

// Passive, committed positions only. No interpolation, map hit targets,
// timers, polling, or raw engine identities in DOM.
export function OperationMapMarkers({markers,camera,scale,crop={x:0,y:0,...MAP_VIEWBOX,scale},mode='rail'}:{markers:unknown;camera:Camera;scale:number;crop?:ReturnType<typeof mapViewport>;mode?:'rail'|'bus'}){
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
  return [{x,y,key}];
 });
 if(!relevant.length)return null;
 return <g pointerEvents="none" aria-label={mode==='bus'?'表示できる走行中のバス':'表示できる走行中の列車'}>
  {relevant.map(({x,y,key})=><circle key={key} cx={x} cy={y} r={radius} fill={mode==='bus'?'#007383':'#1453a0'} stroke="white" strokeWidth="1.5" vectorEffect="non-scaling-stroke"/>)}
 </g>;
}
