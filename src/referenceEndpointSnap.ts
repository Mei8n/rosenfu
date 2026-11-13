import {prepareReferenceCopy,type ReferenceBuild} from './referenceCopy';
import type {ReferenceGraphPick} from './referenceGraphPath';
import type {ReferenceGraphBundle} from './referenceGraphBundle';
import type {GeoPoint} from './alignment';
/** Recompute source fraction, never translate geometry. Exact reproduction of the
 * existing endpoint is required, so nearby parallel/off-source tracks are excluded. */
export function snapReferenceStart(bundle:ReferenceGraphBundle,picks:ReferenceGraphPick[],point:GeoPoint,name:string):ReferenceBuild|null {
 if(picks.length!==2)return null;
 const edge=bundle.graph.edges.find(edge=>edge.id===picks[0].edgeId);
 if(!edge)return null;
 const a=bundle.graph.nodes.find(node=>node.osmId===edge.from)!,b=bundle.graph.nodes.find(node=>node.osmId===edge.to)!;
 const dx=b.longitude-a.longitude,dy=b.latitude-a.latitude;
 const fraction=Math.abs(dx)>=Math.abs(dy)?(point.longitude-a.longitude)/dx:(point.latitude-a.latitude)/dy;
 if(!Number.isFinite(fraction)||fraction<0||fraction>1)return null;
 try {const build=prepareReferenceCopy(bundle,{...picks[0],fraction},picks[1],name),origin=build.points[0];
 return origin.latitude===point.latitude&&origin.longitude===point.longitude?build:null;
 }catch{return null;}
}
