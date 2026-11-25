import {mapViewport,viewBounds,type Camera} from './map';
import type {GeoPoint} from './alignment';

type Bounds=ReturnType<typeof viewBounds>;
export function trackBounds(points:readonly GeoPoint[]):Bounds{
 let west=Infinity,east=-Infinity,north=-Infinity,south=Infinity;
 for(const p of points){
  west=Math.min(west,p.longitude);east=Math.max(east,p.longitude);
  south=Math.min(south,p.latitude);north=Math.max(north,p.latitude);
 }
 return {west,east,north,south};
}
export function trackViewport(camera:Camera,size:{width:number;height:number}):Bounds{
 // Cover the hit stroke and offset track/casing at either SVG or screen scale.
 return viewBounds(camera,size,64+16/mapViewport(size.width,size.height).scale);
}
/** Bounding boxes reject only definitely invisible geometry. A long segment
 * with both vertices outside can still cross the entire view. */
export function intersectsTrackView(bounds:Bounds,view:Bounds):boolean{
 return bounds.west<=view.east&&bounds.east>=view.west&&bounds.north>=view.south&&bounds.south<=view.north;
}


const trackCell=(degrees:number)=>Math.floor(degrees*4);
const trackCellKey=(x:number,y:number)=>x*1024+y;
/** Screen-only broad phase. Small lines occupy at most four cells, and
 * long ones stay in one overflow list, so crossings are never lost. */
export function createTrackViewportIndex<T extends {points:readonly GeoPoint[]}>(items:readonly T[]){
 const rows=items.map(item=>({item,bounds:trackBounds(item.points)})).filter(({bounds:b})=>
  b.west>=-180&&b.east<=180&&b.south>=-85.05112878&&b.north<=85.05112878&&
  b.west<=b.east&&b.south<=b.north);
 const cells=new Map<number,number[]>(),wide:number[]=[];
 const extents=(b:Bounds):[number,number,number,number]=>[trackCell(b.west),trackCell(b.east),trackCell(b.south),trackCell(b.north)];
 if(rows.length>=256)rows.forEach(({bounds},i)=>{
  const [left,right,bottom,top]=extents(bounds);
  if(right-left>1||top-bottom>1){wide.push(i);return;}
  for(let x=left;x<=right;x++)for(let y=bottom;y<=top;y++){
   const key=trackCellKey(x,y),bucket=cells.get(key);
   if(bucket)bucket.push(i);else cells.set(key,[i]);
  }
 });
 const linear=(view:Bounds)=>rows.filter(r=>intersectsTrackView(r.bounds,view)).map(r=>r.item);
 return {visible(view:Bounds):T[]{
  if(!cells.size||wide.length*2>=rows.length||
     view.west< -180||view.east>180||view.south< -90||view.north>90||
     !(view.west<=view.east&&view.south<=view.north))return linear(view);
  const [left,right,bottom,top]=extents(view);
  if((right-left+1)*(top-bottom+1)>256)return linear(view);
  const candidates=new Set(wide);
  for(let x=left;x<=right;x++)for(let y=bottom;y<=top;y++)
   for(const i of cells.get(trackCellKey(x,y))??[])candidates.add(i);
  if(candidates.size*2>=rows.length)return linear(view);
  // Final bounds test is identical to the unindexed path, with stable SVG order.
  const result:T[]=[];
  for(const i of [...candidates].sort((a,b)=>a-b)){
   const row=rows[i];
   if(intersectsTrackView(row.bounds,view))result.push(row.item);
  }
  return result;
 }};
}
