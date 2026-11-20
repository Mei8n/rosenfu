import {project,type Camera} from './map';
import './RouteStationSelection.css';

type Station={id:string;name:string;latitude:number;longitude:number};
/** Draft constraints only: inferred intermediate stations are not explicit picks.
 * Overlay never takes pointer events away from the existing station hit areas. */
export function RouteStationSelection({stations,ordered,camera,scale,entityLabel="路線",endpoint}:{stations:Station[];ordered:string[];camera:Camera;scale:number;entityLabel?:string;endpoint?:"start"|"end"|null}){
 const byId=new Map(stations.map(station=>[station.id,station]));
 return <g className="route-station-selection" pointerEvents="none" aria-label={`${entityLabel}の選択駅`}>
  {ordered.map((id,index)=>{
   const station=byId.get(id);if(!station)return null;
   const point=project(station.latitude,station.longitude,camera);
   const role=index===0?'始点':index===ordered.length-1?'終点':'経由';
   const label=`${index+1} ${role}`;
   const picking=endpoint==="start"?index===0:endpoint==="end"?index===ordered.length-1:false;
   return <g key={id} className={picking?"route-selected-station route-endpoint-picking":"route-selected-station"} aria-label={`${label} ${station.name}`} transform={`translate(${point.x} ${point.y}) scale(${1/scale})`}>
    <circle className="route-selection-halo" r="12"/>
    <path className="route-selection-leader" d="M0,-12 V-22"/>
    <rect className="route-selection-label" x="-29" y="-42" width="58" height="21" rx="5"/>
    <text className="route-selection-text" x="0" y="-27" textAnchor="middle">{label}</text>
   </g>;
  })}
 </g>;
}
