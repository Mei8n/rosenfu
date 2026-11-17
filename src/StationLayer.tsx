import {memo} from 'react';
import type {World} from './App';
import type {GeoPoint} from './alignment';
import type {StationGroupView} from './stationGroups';
import type {routeAppearance} from './routeAppearance';
import {project,type Camera} from './map';

type Props={
  stations:World['stations']|undefined;
  placements:World['station_placements'];
  stationPolygons:Map<string,GeoPoint[]|null>;
  stationGroupDisplay:StationGroupView[];
  camera:Camera;
  selected:string|null;
  routeVisual:ReturnType<typeof routeAppearance>;
  placing:boolean;
  detailedStations:boolean;
  stationNamesVisible:boolean;
  onSelect:(id:string)=>void;
};
export const StationLayer=memo(function StationLayer({stations,placements,stationPolygons,stationGroupDisplay,camera,selected,routeVisual,placing,detailedStations,stationNamesVisible,onSelect}:Props){
  const stationDisplayGroup=(id:string)=>stationGroupDisplay.find(g=>g.members.some(m=>m.id===id));
  const mergedStationIds=new Set(stationGroupDisplay.filter(g=>g.members.length>1).flatMap(g=>g.members.map(m=>m.id)));
  const drawPath=(points:GeoPoint[])=>points.map((point,index)=>{const p=project(point.latitude,point.longitude,camera);return `${index?'L':'M'}${p.x},${p.y}`;}).join(' ');
  return <>
    {stations?.map((station) => {
      const { x, y } = project(station.latitude, station.longitude, camera);
      const placement = placements?.find((p) => p.station_id === station.id);
      const polygon = stationPolygons.get(station.id);
      return <g key={station.id} pointerEvents={placing?"none":undefined} className={"station-marker" + (selected === station.id ? " selected" : "") + (routeVisual.stations.has(station.id) ? "" : " route-unassigned")} onClick={(event) => { event.stopPropagation(); onSelect(station.id); }}>
        <title>{station.name}{placement ? ` / ホーム ${placement.length_m}m` : ""}</title>{!detailedStations ? <circle visibility={stationDisplayGroup(station.id)&&stationDisplayGroup(station.id)?.id!==station.id?"hidden":undefined} cx={x} cy={y} r="5" className="station-dot" /> : polygon ? <path d={drawPath(polygon) + "Z"} className="station-platform" style={mergedStationIds.has(station.id)?{fill:"transparent",stroke:"none"}:undefined} /> : <rect x={x - 13} y={y - 5} width="26" height="10" className="station-platform" />}{stationNamesVisible&&(!stationDisplayGroup(station.id)||stationDisplayGroup(station.id)?.id===station.id)&&<text x={x + 18} y={y + 5}>{station.name}</text>}
      </g>;
    })}
  </>;
});
