import type {GeoPoint} from './alignment';
/** Legacy geometry has no trustworthy anchors: expose one whole-line section. */
export function deletionSections(line:{points:GeoPoint[];anchor_indices?:number[]|null}){
 const values=line.anchor_indices??[0,line.points.length-1];
 if(line.points.length<2||values.length<2||values[0]!==0||values.at(-1)!==line.points.length-1||values.some((n,i)=>!Number.isInteger(n)||n<0||n>=line.points.length||(i>0&&n<=values[i-1])))return [];
 return values.slice(0,-1).map((start,index)=>({index,points:line.points.slice(start,values[index+1]+1)}));
}
export function anchorIndices(anchors:GeoPoint[],line:GeoPoint[]):number[]|undefined{
 let cursor=-1;
 const indices=anchors.map(anchor=>{const index=line.findIndex((point,index)=>index>cursor&&point.latitude===anchor.latitude&&point.longitude===anchor.longitude);cursor=index;return index;});
 return indices.length>=2&&indices[0]===0&&indices.at(-1)===line.length-1&&indices.every((n,i)=>n>=0&&(i===0||n>indices[i-1]))?indices:undefined;
}
