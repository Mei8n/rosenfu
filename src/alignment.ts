/** Provider-independent geographic construction geometry. Distances are metres. */
export type GeoPoint = { latitude: number; longitude: number };
const R = 6371000;
const rad = Math.PI / 180;
export function distanceM(a: GeoPoint, b: GeoPoint): number {
  return R * Math.hypot((b.longitude-a.longitude)*rad*Math.cos((a.latitude+b.latitude)*rad/2), (b.latitude-a.latitude)*rad);
}
export function alignmentLength(points: GeoPoint[]): number {
  return points.slice(1).reduce((length,p,i) => length + distanceM(points[i],p),0);
}
export function pointOnAlignment(points: GeoPoint[], position: number): GeoPoint {
  if (!points.length) throw new Error("Empty alignment");
  let remaining = alignmentLength(points)*Math.max(0,Math.min(1,position));
  for (let i=1;i<points.length;i++) {
    const a=points[i-1], b=points[i], length=distanceM(a,b);
    if (remaining <= length && length>0) { const t=remaining/length; return {latitude:a.latitude+(b.latitude-a.latitude)*t,longitude:a.longitude+(b.longitude-a.longitude)*t}; }
    remaining -= length;
  }
  return points[points.length-1];
}
export function nearestOnAlignment(points: GeoPoint[], target: GeoPoint): {position:number;point:GeoPoint;distanceM:number} {
  const total=alignmentLength(points); let offset=0;
  let best={position:0,point:points[0]??target,distanceM:Infinity};
  for(let i=1;i<points.length;i++) {
    const a=points[i-1],b=points[i],cos=Math.cos((a.latitude+b.latitude)*rad/2);
    const dx=(b.longitude-a.longitude)*cos,dy=b.latitude-a.latitude;
    const tx=(target.longitude-a.longitude)*cos,ty=target.latitude-a.latitude;
    const t=Math.max(0,Math.min(1,(tx*dx+ty*dy)/(dx*dx+dy*dy||1)));
    const point={latitude:a.latitude+(b.latitude-a.latitude)*t,longitude:a.longitude+(b.longitude-a.longitude)*t};
    const length=distanceM(a,b),d=distanceM(point,target);
    if(d<best.distanceM) best={position:total?(offset+length*t)/total:0,point,distanceM:d};
    offset+=length;
  }
  return best;
}
/** Catmull-Rom samples, saved as geometry rather than dependent on a map SDK. */
export function smoothAlignment(controls: GeoPoint[]): GeoPoint[] {
  if(controls.length<3) return controls.map(p=>({...p}));
  const out:GeoPoint[]=[];
  for(let i=0;i<controls.length-1;i++) {
    const a=controls[Math.max(0,i-1)],b=controls[i],c=controls[i+1],d=controls[Math.min(controls.length-1,i+2)];
    for(let step=0;step<16;step++) {
      const t=step/16,t2=t*t,t3=t2*t;
      const value=(key:keyof GeoPoint)=>0.5*((2*b[key])+(-a[key]+c[key])*t+(2*a[key]-5*b[key]+4*c[key]-d[key])*t2+(-a[key]+3*b[key]-3*c[key]+d[key])*t3);
      const p={latitude:value('latitude'),longitude:value('longitude')};
      if(!out.length||distanceM(out[out.length-1],p)>=0.01) out.push(p);
    }
  }
  const last=controls[controls.length-1];
  if(distanceM(out[out.length-1],last)>=0.01) out.push({...last});
  return out;
}
/** Flat-ended ribbon that follows the saved alignment, including curves. */
export function stationPolygon(points:GeoPoint[],position:number,lengthM:number,widthM:number):GeoPoint[] {
  const total=alignmentLength(points);
  if(points.length<2||!Number.isFinite(lengthM)||!Number.isFinite(widthM)||lengthM<=0||widthM<=0||total<lengthM) return [];
  const half=lengthM/total/2,center=Math.max(half,Math.min(1-half,position));
  const start=center-half,end=center+half;
  const spine=[pointOnAlignment(points,start)];
  let offset=0;
  for(let i=1;i<points.length;i++) { offset+=distanceM(points[i-1],points[i]); if(offset/total>start&&offset/total<end) spine.push(points[i]); }
  spine.push(pointOnAlignment(points,end));
  const left:GeoPoint[]=[],right:GeoPoint[]=[];
  for(let i=0;i<spine.length;i++) {
    const p=spine[i],a=spine[Math.max(0,i-1)],b=spine[Math.min(spine.length-1,i+1)];
    const cos=Math.cos(p.latitude*rad),dx=(b.longitude-a.longitude)*cos,dy=b.latitude-a.latitude,n=Math.hypot(dx,dy)||1;
    const latOffset=dx/n*widthM/2/R/rad,lonOffset=-dy/n*widthM/2/R/rad/cos;
    left.push({latitude:p.latitude+latOffset,longitude:p.longitude+lonOffset});
    right.push({latitude:p.latitude-latOffset,longitude:p.longitude-lonOffset});
  }
  return [...left,...right.reverse()];
}