import {createNationalRoadRepository} from './nationalRoadRepository';
import {NATIONAL_ROAD_RELEASE} from './nationalRoadRelease';
import {createRoadRepository,type RoadSnapshot} from './automaticRoadData';
import {createRoadPackRepository} from './roadPackRepository';
import {ROAD_PACK_RELEASE} from './roadPackRelease';
import type {GraphBounds} from './referenceGraph';

export type RoadProvider={name:string;load:(bounds:GraphBounds)=>Promise<RoadSnapshot>};
/** A release selects exactly one provider. Missing or invalid pack coverage must
 * remain unavailable; it never causes a request to a different public service. */
export function createRoadProvider():RoadProvider{
 if(NATIONAL_ROAD_RELEASE){const repository=createNationalRoadRepository(new URL("/road-data/",globalThis.location.origin),NATIONAL_ROAD_RELEASE.indexSha256);return {name:NATIONAL_ROAD_RELEASE.name,load:bounds=>repository.load(bounds)};}
 if(ROAD_PACK_RELEASE!==null){
  const release=ROAD_PACK_RELEASE;
  if(!release.name.trim()||release.name.length>200)throw Error('道路パックの提供名が不正です。');
  const repository=createRoadPackRepository(new URL('/road-data/',globalThis.location.origin),release.manifestSha256);
  return {name:release.name,load:bounds=>repository.load(bounds)};
 }
 const repository=createRoadRepository();
 return {name:'OpenStreetMap / Overpass',load:bounds=>repository.load(bounds)};
}
