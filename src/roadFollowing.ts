import {validateReferenceGraph,type GraphEdge,type ReferenceGraphPack} from './referenceGraph';
import type {ReferenceGraphBundle} from './referenceGraphBundle';
import {extractReferenceWayPath,type ReferenceGraphPick} from './referenceGraphPath';
/** Conservative geometry-only trial. This does not establish right of way,
 * tram suitability, road width, legal access, level, or real-world passability. */
const ROAD_CLASSES=new Set(['primary','secondary','tertiary','unclassified','residential','living_street','service']);
const ALLOWED_ACCESS=new Set(['yes','permissive','designated']);
function eligible(bundle:ReferenceGraphBundle,edge:GraphEdge):boolean{
 const m=bundle.roadWays?.[edge.osmWayId];
 return !!m && edge.mode==='road' && m.highway!==null && ROAD_CLASSES.has(m.highway)
  && (m.access===null||ALLOWED_ACCESS.has(m.access))
  && (m.motorVehicle===null||ALLOWED_ACCESS.has(m.motorVehicle))
  && m.structureUnsupported===false && edge.bridge!==true && edge.tunnel!==true
  && (edge.layer===null||edge.layer===0) && (edge.level===null||/^[+-]?0(?:\.0+)?$/.test(edge.level));
}
/** Filter before nearest-query limits so forbidden nearby edges cannot crowd out
 * an eligible result. Nodes and source segment indices remain unchanged. */
export function eligibleRoadGraph(bundle:ReferenceGraphBundle):ReferenceGraphPack{
 const graph=validateReferenceGraph(bundle.graph);
 return {...graph,edges:graph.edges.filter(edge=>eligible(bundle,edge))};
}
/** Revalidate at selection/build time, including intervening segments. A missing
 * or prohibited middle segment is never bridged or silently straightened. */
export function validateRoadPicks(bundle:ReferenceGraphBundle,picks:ReferenceGraphPick[]):void{
 const graph=eligibleRoadGraph(bundle),edges=new Map(graph.edges.map(edge=>[edge.id,edge]));
 if(picks.length<1||picks.length>2)throw Error('道路の起点・終点を選んでください。');
 let way:string|undefined;
 for(const pick of picks){
  const edge=edges.get(pick.edgeId);
  if(pick.datasetId!==graph.dataset.id||pick.revision!==graph.dataset.revision||!Number.isFinite(pick.fraction)||pick.fraction<0||pick.fraction>1||!edge)throw Error('この道路は限定追従の対象外です。データ・道路種別・階層を確認してください。');
  if(way!==undefined&&way!==edge.osmWayId)throw Error('限定道路追従では同じOSM way上の終点を選んでください。');
  way=edge.osmWayId;
 }
 if(picks.length===2)extractReferenceWayPath(graph,picks[0],picks[1]);
}
