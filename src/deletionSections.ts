import type {GeoPoint} from './alignment';
/** Legacy geometry has no trustworthy anchors: expose one whole-line section. */
export function deletionSections(line:{points:GeoPoint[];anchor_indices?:number[]|null}){
 const values=line.anchor_indices??[0,line.points.length-1];
 if(line.points.length<2||values.length<2||values[0]!==0||values.at(-1)!==line.points.length-1||values.some((n,i)=>!Number.isInteger(n)||n<0||n>=line.points.length||(i>0&&n<=values[i-1])))return [];
 return values.slice(0,-1).map((start,index)=>({index,points:line.points.slice(start,values[index+1]+1)}));
}
export function anchorIndices(anchors:GeoPoint[],line:GeoPoint[]):number[]|undefined{
 if(anchors.length<2||anchors.length>line.length)return undefined;
 const indices:number[]=[];
 let cursor=0;
 // Keep the first forward match, including duplicate coordinates. Forcing
 // the final anchor onto the last point would accept some ambiguous loops.
 for(const anchor of anchors){
  while(cursor<line.length){
   const point=line[cursor];
   if(point.latitude===anchor.latitude&&point.longitude===anchor.longitude)break;
   cursor++;
  }
  if(cursor===line.length||(!indices.length&&cursor!==0))return undefined;
  indices.push(cursor++);
 }
 return indices.at(-1)===line.length-1?indices:undefined;
}
