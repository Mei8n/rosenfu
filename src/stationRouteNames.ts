import {firstByKey} from './mapLookupIndex';

export function stationRouteNames(routes:readonly {id:string;name:string;start:string}[],paths:readonly {route_id:string;member_stations:string[]}[]){
 const namesByStation=new Map<string,Set<string>>();
 const pathByRoute=firstByKey(paths,path=>path.route_id);
 for(const route of routes){
  for(const id of pathByRoute.get(route.id)?.member_stations??[route.start]){
   const names=namesByStation.get(id)??new Set<string>();
   names.add(route.name);namesByStation.set(id,names);
  }
 }
 return namesByStation;
}
