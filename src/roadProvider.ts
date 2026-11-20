import {createOpenFreeMapRoadRepository,type OpenFreeMapEvidenceRequest,type OpenFreeMapEvidenceBatch} from './openFreeMapRoadData';
import type {RoadSnapshot} from './automaticRoadData';
import type {GraphBounds} from './referenceGraph';

export type RoadProvider={name:string;load:(bounds:GraphBounds)=>Promise<RoadSnapshot>;readTileEvidenceBatch:(refs:readonly OpenFreeMapEvidenceRequest[],signal?:AbortSignal)=>Promise<OpenFreeMapEvidenceBatch>};
/** Runtime roads use the provider's current bounded vector tiles. Archived pack
 * generators remain available for research, but are not a silent fallback. */
export function createRoadProvider():RoadProvider{
 const repository=createOpenFreeMapRoadRepository();
 return {name:'OpenFreeMap / OpenStreetMap',load:bounds=>repository.load(bounds),readTileEvidenceBatch:(refs,signal)=>repository.readTileEvidenceBatch(refs,signal)};
}
