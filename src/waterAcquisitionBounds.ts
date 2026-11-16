import type {GeoPoint} from './alignment';
/** A stable local area plus active construction geometry, not a nationwide prefetch. */
export function waterAcquisitionBounds(points:readonly GeoPoint[],camera:{longitude:number;latitude:number}):[number,number,number,number]{
 const xs=[camera.longitude-0.01,camera.longitude+0.01,...points.map(p=>p.longitude)],ys=[camera.latitude-0.01,camera.latitude+0.01,...points.map(p=>p.latitude)];
 const low=(v:number)=>Math.floor(v*20)/20,high=(v:number)=>Math.ceil(v*20)/20;
 return [Math.max(122,low(xs.reduce((a,b)=>Math.min(a,b),Infinity))),Math.max(20,low(ys.reduce((a,b)=>Math.min(a,b),Infinity))),Math.min(154,high(xs.reduce((a,b)=>Math.max(a,b),-Infinity))),Math.min(46,high(ys.reduce((a,b)=>Math.max(a,b),-Infinity)))];
}
