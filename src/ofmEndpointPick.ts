import type {TileRoadLine} from './ofmBoundaryContinuations';
import type {TilePathPick} from './ofmDerivedTilePath';
import type {GeoPoint} from './constructionPlans';
/** Reparameterize a source fragment, never move its coordinates. Ambiguous self-intersections fail. */
export function ofmEndpointPick(lines:readonly TileRoadLine[],pick:TilePathPick,target:GeoPoint):TilePathPick|null{
 const line=lines.find(l=>l.id===pick.fragmentId);if(!line)return null;
 const hits:number[]=[];
 for(let i=0;i<line.coordinates.length-1;i++){
  const a=line.coordinates[i],b=line.coordinates[i+1],dx=b[0]-a[0],dy=b[1]-a[1],length=dx*dx+dy*dy;
  if(!length)continue;
  const fraction=((target.longitude-a[0])*dx+(target.latitude-a[1])*dy)/length;
  if(fraction < -1e-10||fraction > 1+1e-10)continue;
  const t=Math.max(0,Math.min(1,fraction));
  if(Math.abs(a[0]+dx*t-target.longitude)>1e-10||Math.abs(a[1]+dy*t-target.latitude)>1e-10)continue;
  const position=i+t;if(!hits.some(h=>Math.abs(h-position)<1e-9))hits.push(position);
 }
 if(hits.length!==1)return null;
 const segmentIndex=Math.min(Math.floor(hits[0]),line.coordinates.length-2);
 return {fragmentId:pick.fragmentId,segmentIndex,fraction:hits[0]-segmentIndex};
}
