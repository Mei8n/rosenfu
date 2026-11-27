/** Screen-space presentation only. Physical track/platform geometry and picking stay unchanged.
 * Shared zoom response avoids a jump in visual weight when a service is assigned. */
export function mapRenderMetrics(zoom:number, lineWidth:number, viewportScale:number) {
 const scale=Number.isFinite(viewportScale)&&viewportScale>0?viewportScale:1;
 const z=Number.isFinite(zoom)?zoom:12;
 const weight=Math.min(8,Math.max(2,Number.isFinite(lineWidth)?lineWidth:4));
 const zoomWeight=Math.min(1,Math.max(.4,.4+(z-9)*.1));
 const track=(weight*.6+1)*zoomWeight;
 const service=track*1.15;
 const casing=1.2;
 const stationRadius=Math.max(2.4,service*.7);
 return {track,trackCasing:track+casing*2,service:service/scale,
  serviceCasing:(service+casing*2)/scale,stationRadius:stationRadius/scale,
  stationLabelOffset:(stationRadius+5)/scale,stationLabelSize:12/scale,
  stationLabelBaseline:4/scale,stationRim:1.2,scale};
}
