import {alignmentLength,distanceM,pointOnAlignment,type GeoPoint} from './alignment';
import {corridorSpans} from './connectedCorridor';
import {resolveServicePath,type ServicePath,type PathWorld} from './servicePaths';
export type ServiceMapWorld=PathWorld&{
 services:{id:string;start:string;name:string}[];
 service_paths?:ServicePath[];
 alignments?:{id:string;points:GeoPoint[]}[];
 service_appearances?:{service_id:string;color:string}[];
};
type Span={alignmentId:string;begin:number;finish:number};
export type ServiceBand={serviceId:string;color:string;name:string};
export type SharedServiceSection={alignmentId:string;begin:number;finish:number;points:GeoPoint[];bands:ServiceBand[]};
const validColor=(color:string)=>/^#[0-9a-f]{6}$/i.test(color);
function slice(points:GeoPoint[],begin:number,finish:number){
 const total=alignmentLength(points);if(total<=0)return [];
 const result=[pointOnAlignment(points,begin)];let distance=0;
 for(let i=1;i<points.length;i++){distance+=distanceM(points[i-1],points[i]);if(distance/total>begin&&distance/total<finish)result.push(points[i]);}
 result.push(pointOnAlignment(points,finish));return result;
}
/** Uses saved physical service paths, not route names or proximity. Stops do not clip pass-through sections. */
export function sharedServiceSections(world:ServiceMapWorld|null):SharedServiceSection[]{
 if(!world)return [];
 const byAlignment=new Map<string,{span:Span;band:ServiceBand}[]>(),alignments=new Map(world.alignments?.map(a=>[a.id,a]));
 for(const service of world.services){
  const path=world.service_paths?.find(p=>p.service_id===service.id);if(!path)continue;
  const spans:Span[]=[];
  try{
   resolveServicePath(service.start,path.legs,world);
   for(const {segment} of path.legs){
    if(segment.kind==='connected_alignments')spans.push(...corridorSpans(segment,world));
    else{
     const a=world.station_placements?.find(p=>p.station_id===segment.from),b=world.station_placements?.find(p=>p.station_id===segment.to);
     if(!a||!b||a.alignment_id!==segment.alignment_id||b.alignment_id!==segment.alignment_id)throw Error('stale placement');
     spans.push({alignmentId:segment.alignment_id,begin:a.position,finish:b.position});
    }
   }
   if(spans.some(s=>!alignments.has(s.alignmentId)||!Number.isFinite(s.begin)||!Number.isFinite(s.finish)||Math.min(s.begin,s.finish)<0||Math.max(s.begin,s.finish)>1||s.begin===s.finish))throw Error('stale span');
  }catch{continue;} // Fail closed for the entire invalid service, not a misleading partial route.
  const color=world.service_appearances?.find(a=>a.service_id===service.id)?.color??'#3b82f6';
  const band={serviceId:service.id,name:service.name,color:validColor(color)?color:'#3b82f6'};
  for(const span of spans){const rows=byAlignment.get(span.alignmentId)??[];rows.push({span:{...span,begin:Math.min(span.begin,span.finish),finish:Math.max(span.begin,span.finish)},band});byAlignment.set(span.alignmentId,rows);}
 }
 const result:SharedServiceSection[]=[];
 for(const [alignmentId,rows] of byAlignment){
  const boundaries=[...new Set(rows.flatMap(({span})=>[span.begin,span.finish]))].sort((a,b)=>a-b);
  for(let i=1;i<boundaries.length;i++){
   const begin=boundaries[i-1],finish=boundaries[i],middle=(begin+finish)/2;
   const bands=[...new Map(rows.filter(({span})=>span.begin<=middle&&span.finish>=middle).map(({band})=>[band.serviceId,band])).values()].sort((a,b)=>a.serviceId.localeCompare(b.serviceId,undefined,{numeric:true}));
   if(!bands.length)continue;
   const previous=result.at(-1);
   if(previous?.alignmentId===alignmentId&&previous.finish===begin&&previous.bands.map(b=>b.serviceId).join(',')===bands.map(b=>b.serviceId).join(',')){
    previous.finish=finish;previous.points=slice(alignments.get(alignmentId)!.points,previous.begin,finish);
   }else result.push({alignmentId,begin,finish,points:slice(alignments.get(alignmentId)!.points,begin,finish),bands});
  }
 }
 return result;
}
/** Equal-width longitudinal colour stripes. This is a service overlay, never physical track count. */
export function serviceBandPaths(points:{x:number;y:number}[],bands:ServiceBand[],width=8){
 const stripe=width/bands.length;
 const closed=points.length>2&&points[0].x===points.at(-1)!.x&&points[0].y===points.at(-1)!.y;
 return bands.map((band,index)=>{
  const offset=(index-(bands.length-1)/2)*stripe;
  const d=points.map((p,i)=>{
   const seam=closed&&(i===0||i===points.length-1);
   const a=seam?points[points.length-2]:points[Math.max(0,i-1)],b=seam?points[1]:points[Math.min(points.length-1,i+1)],dx=b.x-a.x,dy=b.y-a.y,n=Math.hypot(dx,dy);
   return (i?'L':'M')+(p.x+(n?-dy/n*offset:0))+','+(p.y+(n?dx/n*offset:0));
  }).join(' ');
  return {...band,d,width:stripe};
 });
}

export type ServiceRenderChain={alignmentIds:string[];points:GeoPoint[];bands:ServiceBand[]};
/** Join presentation only, and only across explicit two-ended connections.
 * A plan boundary must not restart stripe offsets or create a visible cap.
 * Branches, unconnected crossings and changes in the set of services remain separate. */
export function serviceRenderChains(sections:SharedServiceSection[],connections:NonNullable<ServiceMapWorld['alignment_connections']>):ServiceRenderChain[]{
 const endpointKey=(id:string,end:'start'|'end')=>`${id}:${end}`;
 const links=new Map<string,string[]>();
 for(const {a,b} of connections){
  const left=endpointKey(a.alignment_id,a.endpoint),right=endpointKey(b.alignment_id,b.endpoint);
  links.set(left,[...new Set([...(links.get(left)??[]),right])]);
  links.set(right,[...new Set([...(links.get(right)??[]),left])]);
 }
 const ends=new Map<string,{index:number;end:0|1}[]>();
 sections.forEach((section,index)=>{
  for(const [end,position,label] of [[0,section.begin,'start'],[1,section.finish,'end']] as const){
   if(position!==(end===0?0:1))continue;
   const key=endpointKey(section.alignmentId,label);
   ends.set(key,[...(ends.get(key)??[]),{index,end}]);
  }
 });
 const signature=(s:SharedServiceSection)=>JSON.stringify(s.bands.map(b=>[b.serviceId,b.color]));
 const joins=new Map<string,{index:number;end:0|1}>();
 for(const [key,rows] of ends){
  const peers=links.get(key);if(rows.length!==1||peers?.length!==1||links.get(peers[0])?.length!==1)continue;
  const targets=ends.get(peers[0]);if(targets?.length!==1)continue;
  const a=rows[0],b=targets[0];if(a.index===b.index||signature(sections[a.index])!==signature(sections[b.index]))continue;
  const ap=a.end===0?sections[a.index].points[0]:sections[a.index].points.at(-1);
  const bp=b.end===0?sections[b.index].points[0]:sections[b.index].points.at(-1);
  if(!ap||!bp||distanceM(ap,bp)>0.01)continue;
  joins.set(`${a.index}:${a.end}`,b);
 }
 const visited=new Set<number>(),result:ServiceRenderChain[]=[];
 function walk(index:number,end:0|1){
  const firstIndex=index,firstEnd=end;
  const chain:ServiceRenderChain={alignmentIds:[],points:[],bands:sections[index].bands};
  while(!visited.has(index)){
   visited.add(index);const section=sections[index];
   const points=end===0?section.points:[...section.points].reverse();
   for(let point=chain.points.length?1:0;point<points.length;point++)chain.points.push(points[point]);
   chain.alignmentIds.push(section.alignmentId);
   const next=joins.get(`${index}:${end===0?1:0}`);if(!next)break;
   if(next.index===firstIndex&&next.end===firstEnd){chain.points[chain.points.length-1]=chain.points[0];break;}
   index=next.index;end=next.end;
  }
  result.push(chain);
 }
 // Start open chains at an unjoined end; closed circuits are handled afterwards.
 sections.forEach((_,i)=>{if(visited.has(i))return;if(!joins.has(`${i}:0`))walk(i,0);else if(!joins.has(`${i}:1`))walk(i,1);});
 sections.forEach((_,i)=>{if(!visited.has(i))walk(i,0);});
 return result;
}
