import type {GeoPoint} from './alignment';
/** Match the core extension join tolerance; ordinary independent lines are unaffected. */
export function extensionJoinError(points:GeoPoint[],previous?:GeoPoint):string|null {
 if(!previous||points.length<2)return null;
 const anchor=points[0],next=points[1],scale=Math.cos(anchor.latitude*Math.PI/180);
 const ix=(anchor.longitude-previous.longitude)*scale,iy=anchor.latitude-previous.latitude;
 const ox=(next.longitude-anchor.longitude)*scale,oy=next.latitude-anchor.latitude;
 const norm=Math.hypot(ix,iy)*Math.hypot(ox,oy);
 return !Number.isFinite(norm)||norm<=0||(ix*ox+iy*oy)/norm<Math.cos(5*Math.PI/180)
 ?'既存線路との接合角は5度以内にしてください。曲線モードなら接線方向を引き継いで延伸できます':null;
}
