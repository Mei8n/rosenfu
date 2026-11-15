import type {GraphBounds,GraphPoint} from './referenceGraph';
import type {EnvironmentSnapshot} from './environmentData';
import type {RoadSnapshot} from './automaticRoadData';
export const CORRIDOR_LIMITS={regions:128,vertices:4096,features:10000,points:100000} as const;
/** Fixed cells visited by the line, not its potentially enormous bounding rectangle. */
export function corridorBounds(lines:readonly (readonly GraphPoint[])[]):GraphBounds[]{
 const cells=new Map<string,GraphBounds>();let vertices=0;
 const add=(x:number,y:number)=>{const key=`${x}/${y}`;if(!cells.has(key)){if(cells.size>=CORRIDOR_LIMITS.regions)throw Error('計画回廊の取得上限です。計画を分割してください。');cells.set(key,{west:x/50,east:(x+1)/50,south:y/50,north:(y+1)/50});}};
 for(const line of lines){
  vertices+=line.length;if(vertices>CORRIDOR_LIMITS.vertices)throw Error('計画回廊の頂点上限です。');
  for(const p of line)if(!Number.isFinite(p.longitude)||!Number.isFinite(p.latitude)||Math.abs(p.longitude)>=180||Math.abs(p.latitude)>=85)throw Error('計画回廊の座標が不正です。');
  for(let i=1;i<line.length;i++){
   const a=line[i-1],b=line[i],dx=b.longitude-a.longitude,dy=b.latitude-a.latitude;
   const steps=Math.max(1,Math.ceil(Math.max(Math.abs(dx),Math.abs(dy))*100));
   if(steps>CORRIDOR_LIMITS.regions*2)throw Error('計画回廊の取得上限です。');
   for(let k=0;k<steps;k++){
    const x0=Math.floor((a.longitude+dx*k/steps)*50),x1=Math.floor((a.longitude+dx*(k+1)/steps)*50);
    const y0=Math.floor((a.latitude+dy*k/steps)*50),y1=Math.floor((a.latitude+dy*(k+1)/steps)*50);
    for(let y=Math.min(y0,y1);y<=Math.max(y0,y1);y++)for(let x=Math.min(x0,x1);x<=Math.max(x0,x1);x++)add(x,y);
   }
  }
 }
 return [...cells.values()];
}
export function combineCorridor(parts:{road:RoadSnapshot;water:{coverage:GraphBounds[];water:EnvironmentSnapshot['water']}}[]):EnvironmentSnapshot{
 const unique=(bs:GraphBounds[])=>[...new Map(bs.map(b=>[JSON.stringify(b),b])).values()];
 const road_coverage=unique(parts.map(p=>p.road.bounds)),water_coverage=unique(parts.flatMap(p=>p.water.coverage));
 const roads=parts.flatMap(p=>p.road.roads.map(r=>({...r,coverage:[p.road.bounds]})));
 const polygons=new Map<string,EnvironmentSnapshot['water'][number]>();
 for(const p of parts)for(const w of p.water.water){const key=JSON.stringify(w.rings),prior=polygons.get(key);polygons.set(key,{...w,coverage:unique([...(prior?.coverage??[]),...p.water.coverage])});}
 const water=[...polygons.values()];
 const points=roads.reduce((n,r)=>n+r.points.length,0)+water.reduce((n,w)=>n+w.rings.reduce((m,r)=>m+r.length,0),0);
 if(road_coverage.length>128||water_coverage.length>128||roads.length>10000||water.length>10000||points>100000||roads.length+water.reduce((n,w)=>n+(w.coverage?.length??0),0)>10000)throw Error('計画回廊の保存上限です。計画を分割してください。');
 return {revision:parts[0]?.road.bundle.graphSha256??'',road_coverage,water_coverage,roads,water};
}
