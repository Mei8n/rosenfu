import {memo} from 'react';
import type {World} from './App';
import type {GeoPoint} from './alignment';
import type {StationGroupView} from './stationGroups';
import type {routeAppearance} from './routeAppearance';
import {project,type Camera} from './map';

type Props={
  rows:{station:World['stations'][number]}[];
  placements:Map<string,NonNullable<World['station_placements']>[number]>;
  stationPolygons:Map<string,GeoPoint[]|null>;
  groupByMember:Map<string,StationGroupView>;
  mergedStationIds:Set<string>;
  hitRadius:number;
  dotRadius:number;
  labelOffset:number;
  labelBaseline:number;
  camera:Camera;
  selected:string|null;
  routeVisual:ReturnType<typeof routeAppearance>;
  placing:boolean;
  detailedStations:boolean;
  stationNamesVisible:boolean;
  onSelect:(id:string)=>void;
};
export const StationLayer=memo(function StationLayer({rows,placements,stationPolygons,groupByMember,mergedStationIds,hitRadius,dotRadius,labelOffset,labelBaseline,camera,selected,routeVisual,placing,detailedStations,stationNamesVisible,onSelect}:Props){
  const drawPath=(points:GeoPoint[])=>points.map((point,index)=>{const p=project(point.latitude,point.longitude,camera);return `${index?'L':'M'}${p.x},${p.y}`;}).join(' ');
  return <>
    {rows.map(({station}) => {
      const { x, y } = project(station.latitude, station.longitude, camera);
      const placement = placements.get(station.id);
      const polygon = stationPolygons.get(station.id);
      const group=groupByMember.get(station.id), hideDot=group&&group.id!==station.id?"hidden":undefined;
      return <g key={station.id} data-station-id={station.id} pointerEvents={placing?"none":undefined} className={"station-marker" + (selected === station.id ? " selected" : "") + (routeVisual.stations.has(station.id) ? "" : " route-unassigned")} onClick={(event) => { event.stopPropagation(); onSelect(station.id); }}>
        <title>{station.name}{placement ? ` / ホーム ${placement.length_m}m` : ""}</title>{!detailedStations&&<circle className="station-hit-target" aria-hidden="true" visibility={hideDot} cx={x} cy={y} r={hitRadius} fill="transparent" stroke="none"/>}{!detailedStations ? <circle visibility={hideDot} cx={x} cy={y} r={dotRadius} className="station-dot" /> : polygon ? <path d={drawPath(polygon) + "Z"} className="station-platform" style={mergedStationIds.has(station.id)?{fill:"transparent",stroke:"none"}:undefined} /> : <rect x={x - 13} y={y - 5} width="26" height="10" className="station-platform" />}{stationNamesVisible&&!hideDot&&<text x={x + labelOffset} y={y + labelBaseline}>{station.name}</text>}
      </g>;
    })}
  </>;
});
