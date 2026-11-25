import {distanceM,pointOnAlignment,type GeoPoint} from './alignment';
import type {RoadTransportData} from './RoadTransportPanel';

export type TransferStation={id:string;name:string}&Partial<GeoPoint>;
export type TransferRoad=Pick<RoadTransportData,'edges'|'bus_routes'>;
export const TRANSFER_CANDIDATE_RADIUS_M=800;
const MAX_CANDIDATES=4;

function validPoint(point:Partial<GeoPoint>|undefined):point is GeoPoint{
 return typeof point?.latitude==='number'&&Number.isFinite(point.latitude)&&Math.abs(point.latitude)<=90&&
  typeof point.longitude==='number'&&Number.isFinite(point.longitude)&&Math.abs(point.longitude)<=180;
}

/** Geographic hints only, based on a stop occurrence from the committed route.
 * Never infer a missing road position or a viable pedestrian connection. */
export function nearbyTransferStations(stations:readonly TransferStation[],road:TransferRoad|undefined,
 stop:Pick<RoadTransportData['bus_stops'][number],'id'|'edge_id'|'position'>|undefined){
 if(!road||!stop||!Number.isFinite(stop.position)||stop.position<0||stop.position>1)return [];
 if(!road.bus_routes.some(route=>route.stops.some(occurrence=>
  occurrence.stop_id===stop.id&&route.edges[occurrence.edge_index]===stop.edge_id)))return [];
 const edge=road.edges.find(candidate=>candidate.id===stop.edge_id);
 if(!edge||edge.geometry.length<2||edge.geometry.some((point,i)=>!validPoint(point)||
  i>0&&Math.abs(point.longitude-edge.geometry[i-1].longitude)>180))return [];
 const point=pointOnAlignment(edge.geometry,stop.position);
 if(!validPoint(point))return [];
 const candidates:{id:string;name:string;distanceM:number}[]=[];
 for(const station of stations){
  if(station.id.startsWith('bus:')||!validPoint(station))continue;
  const distance=distanceM(point,station);
  if(!Number.isFinite(distance)||distance>TRANSFER_CANDIDATE_RADIUS_M)continue;
  candidates.push({id:station.id,name:station.name,distanceM:distance});
 }
 return candidates.sort((a,b)=>a.distanceM-b.distanceM||a.name.localeCompare(b.name,'ja')||
  a.id.localeCompare(b.id)).slice(0,MAX_CANDIDATES);
}
