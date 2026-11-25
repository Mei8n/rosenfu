import {createOpenFreeMapRoadRepository,type OpenFreeMapEvidenceRequest,type OpenFreeMapEvidenceBatch} from './openFreeMapRoadData';
import type {RoadSnapshot} from './automaticRoadData';
import type {GraphBounds} from './referenceGraph';

export type RoadProvider={name:string;load:(bounds:GraphBounds)=>Promise<RoadSnapshot>;readTileEvidenceBatch:(refs:readonly OpenFreeMapEvidenceRequest[],signal?:AbortSignal)=>Promise<OpenFreeMapEvidenceBatch>;readPinnedSeamTiles:(refs:readonly OpenFreeMapEvidenceRequest[],signal?:AbortSignal)=>Promise<OpenFreeMapEvidenceBatch>;observeTiles:(cells:readonly (readonly [number,number])[],revision:string,signal?:AbortSignal)=>Promise<OpenFreeMapEvidenceRequest[]>};
/** Runtime roads use the provider's current bounded vector tiles. Archived pack
 * generators remain available for research, but are not a silent fallback. */
export function createRoadProvider():RoadProvider{
 const repository=createOpenFreeMapRoadRepository();
 return {name:'OpenFreeMap / OpenStreetMap',load:bounds=>repository.load(bounds),readTileEvidenceBatch:(refs,signal)=>repository.readTileEvidenceBatch(refs,signal),readPinnedSeamTiles:(refs,signal)=>repository.readPinnedSeamTiles(refs,signal),observeTiles:(cells,revision,signal)=>repository.observeTiles(cells,revision,signal)};
}
