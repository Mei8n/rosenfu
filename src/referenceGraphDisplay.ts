import {createReferenceSpatialIndex} from "./referenceSpatialIndex.ts";
import type {GraphBounds,GraphEdge,GraphNode,ReferenceGraphPack} from './referenceGraph';
export const GRAPH_DISPLAY_LIMIT=1500;
export const GRAPH_DISPLAY_MIN_ZOOM=14;
export type DisplayEdge={edge:GraphEdge;from:GraphNode;to:GraphNode;bounds:GraphBounds};
const spatialIndexes=new WeakMap<DisplayEdge[],ReturnType<typeof createReferenceSpatialIndex>>();
/** Geometry-only index: no smoothing, connectivity inference or World mutation. */
export function indexGraphDisplay(graph:ReferenceGraphPack):DisplayEdge[]{
 const nodes=new Map(graph.nodes.map(n=>[n.osmId,Object.freeze({...n})]));
 const result=graph.edges.map(edge=>{const from=nodes.get(edge.from)!,to=nodes.get(edge.to)!;return Object.freeze({edge:Object.freeze({...edge}),from,to,bounds:Object.freeze({west:Math.min(from.longitude,to.longitude),east:Math.max(from.longitude,to.longitude),south:Math.min(from.latitude,to.latitude),north:Math.max(from.latitude,to.latitude)})});});
 spatialIndexes.set(result,createReferenceSpatialIndex(result.map(e=>e.bounds)));
 Object.freeze(result);return result;
}
export function queryGraphDisplay(index:DisplayEdge[],bounds:GraphBounds,zoom:number,visible:boolean){
 const edges:DisplayEdge[]=[];let total=0;if(!visible||zoom<GRAPH_DISPLAY_MIN_ZOOM)return {edges,total};
 const spatial=spatialIndexes.get(index);
 const candidates=spatial?spatial.query(bounds).map(i=>index[i]):index;
 for(const item of candidates){const b=item.bounds;if(b.east<bounds.west||b.west>bounds.east||b.north<bounds.south||b.south>bounds.north)continue;total++;if(edges.length<GRAPH_DISPLAY_LIMIT)edges.push(item);}
 return {edges,total};
}
