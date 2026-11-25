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
