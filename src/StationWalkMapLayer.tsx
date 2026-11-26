import {memo} from 'react';
import {project,type Camera} from './map';
import {stationWalkDuration,type StationWalkLink} from './StationWalkLinks';

type Station={id:string;name:string;latitude:number;longitude:number};
type Passage={station:Station;outgoing?:number;incoming?:number};
type View={x:number;y:number;width:number;height:number;scale:number};
export const MAX_VISIBLE_STATION_PASSAGES=12;

/** Resolve only saved links of the selected station; keep opposite directions
 * together without choosing a common duration or inventing a new link. */
export function selectedStationPassages(selected:string,links:readonly StationWalkLink[],stations:ReadonlyMap<string,Station>){
 const pairs=new Map<string,Passage>();
 for(const link of links){
  const outbound=link.from===selected;
  const partner=outbound?link.to:link.to===selected?link.from:null;
  if(!partner||partner===selected)continue;
  const station=stations.get(partner);
  if(!station)continue;
  const pair=pairs.get(partner)??{station};
  if(outbound)pair.outgoing=link.seconds;
  else pair.incoming=link.seconds;
  pairs.set(partner,pair);
 }
 return [...pairs.values()];
}

function directionLabel(from:Station,to:Station,seconds:number){
 return `${from.name} から ${to.name}（${stationWalkDuration(seconds)}）`;
}

function Waypoint({x,y,dx,dy,seconds,scale,labelY}:{x:number;y:number;dx:number;dy:number;seconds:number;scale:number;labelY:number}){
 return <g transform={`translate(${x} ${y}) scale(${1/scale})`} className="station-walk-waypoint">
  <path className="station-walk-arrow" d="M -6 -4 L 3.5 0 L -6 4 Z"
   transform={`rotate(${Math.atan2(dy,dx)*180/Math.PI})`} fill="#673376" stroke="white" strokeWidth="1"/>
  <rect x="-25" y={labelY-7} width="50" height="14" rx="4" fill="white" stroke="#673376"/>
  <text x="0" y={labelY+0.5} textAnchor="middle" dominantBaseline="middle" fontSize="10" fontWeight="bold" fill="#532762">{stationWalkDuration(seconds)}</text>
 </g>;
}

export const StationWalkMapLayer=memo(function StationWalkMapLayer({selected,camera,viewport,passages}:{
 selected:Station;camera:Camera;viewport:View;passages:readonly Passage[];
}){
 if(!passages.length||!Number.isFinite(viewport.scale)||viewport.scale<=0)return null;
 const start=project(selected.latitude,selected.longitude,camera);
 const {x,y,width,height,scale}=viewport;
 const pairs=[];
 for(const pair of passages){
  const end=project(pair.station.latitude,pair.station.longitude,camera);
  if(Math.max(start.x,end.x)<x||Math.min(start.x,end.x)>x+width||
   Math.max(start.y,end.y)<y||Math.min(start.y,end.y)>y+height)continue;
  const dx=end.x-start.x,dy=end.y-start.y;
  if(!Number.isFinite(dx)||!Number.isFinite(dy)||Math.hypot(dx,dy)*scale<14)continue;
  const point=(fraction:number)=>({x:start.x+dx*fraction,y:start.y+dy*fraction});
  const two=pair.outgoing!==undefined&&pair.incoming!==undefined;
  const outgoing=point(two?0.35:0.5),incoming=point(two?0.65:0.5);
  const title=[pair.outgoing===undefined?'':directionLabel(selected,pair.station,pair.outgoing),
   pair.incoming===undefined?'':directionLabel(pair.station,selected,pair.incoming)].filter(Boolean).join(' / ');
  pairs.push(<g key={pair.station.id} className="station-walk-passage" aria-label={title}>
   <title>{title}</title>
   <line x1={start.x} y1={start.y} x2={end.x} y2={end.y} stroke="white" strokeWidth="5" opacity="0.95" vectorEffect="non-scaling-stroke"/>
   <line x1={start.x} y1={start.y} x2={end.x} y2={end.y} stroke="#673376" strokeWidth="2"
    strokeDasharray="4 4" vectorEffect="non-scaling-stroke"/>
   {pair.outgoing!==undefined&&<Waypoint {...outgoing} dx={dx} dy={dy} labelY={13} seconds={pair.outgoing} scale={scale}/>}
   {pair.incoming!==undefined&&<Waypoint {...incoming} dx={-dx} dy={-dy} labelY={-13} seconds={pair.incoming} scale={scale}/>}
  </g>);
  if(pairs.length===MAX_VISIBLE_STATION_PASSAGES)break;
 }
 return pairs.length?<g className="station-walk-map-layer" aria-label="駅間の徒歩連絡（概略表示・実際の徒歩経路ではありません）" pointerEvents="none">{pairs}</g>:null;
});
