import {project,type Camera} from './map';
import {deletionSections} from './deletionSections';
import type {GeoPoint} from './alignment';
export type SelectionPoint={x:number;y:number};
export type SelectionBox={start:SelectionPoint;end:SelectionPoint};
export type SectionSelection={alignmentId:string;sectionIndices:number[]};
export function selectionBounds(box:SelectionBox){return {left:Math.min(box.start.x,box.end.x),right:Math.max(box.start.x,box.end.x),top:Math.min(box.start.y,box.end.y),bottom:Math.max(box.start.y,box.end.y)};}
// Liang–Barsky clipping includes a segment crossing a box even when both endpoints are outside.
function intersects(a:SelectionPoint,b:SelectionPoint,box:ReturnType<typeof selectionBounds>){
 let lo=0,hi=1;const dx=b.x-a.x,dy=b.y-a.y;
 for(const [p,q] of [[-dx,a.x-box.left],[dx,box.right-a.x],[-dy,a.y-box.top],[dy,box.bottom-a.y]]){
  if(p===0){if(q<0)return false;continue;}const t=q/p;if(p<0)lo=Math.max(lo,t);else hi=Math.min(hi,t);if(lo>hi)return false;
 }return true;
}
export function selectAlignmentSections(lines:{id:string;owner:string;points:GeoPoint[];anchor_indices?:number[]|null}[],owner:string|undefined,box:SelectionBox,camera:Camera):SectionSelection[]{
 const bounds=selectionBounds(box);return lines.filter(line=>line.owner===owner).flatMap(line=>{
  const sectionIndices=deletionSections(line).filter(section=>{const points=section.points.map(p=>project(p.latitude,p.longitude,camera));return points.slice(1).some((p,i)=>intersects(points[i],p,bounds));}).map(section=>section.index);
  return sectionIndices.length?[{alignmentId:line.id,sectionIndices}]:[];
 });
}
