import type {GeoPoint} from './constructionPlans';
export type SectionInfrastructure={track_count:1|2|4;level:'ground'|'elevated'|'elevated_2'|'elevated_3'|'underground'|'underground_2'|'underground_3'};
export const DEFAULT_INFRASTRUCTURE:SectionInfrastructure={track_count:1,level:'ground'};
export const LEVEL_LABELS={ground:'地上',elevated:'高架1',elevated_2:'高架2',elevated_3:'高架3',underground:'地下1',underground_2:'地下2',underground_3:'地下3'} as const;
export const COUNT_LABELS={1:'単線',2:'複線',4:'複々線'} as const;
export function infrastructureSections(alignment:{points:GeoPoint[];anchor_indices?:number[]|null;infrastructure?:SectionInfrastructure[]}){
 const boundaries=alignment.anchor_indices??[0,alignment.points.length-1];
 return boundaries.slice(0,-1).map((start,index)=>({points:alignment.points.slice(start,boundaries[index+1]+1),infrastructure:alignment.infrastructure?.[index]??DEFAULT_INFRASTRUCTURE}));
}
export function endpointInfrastructure(alignment:Parameters<typeof infrastructureSections>[0],endpoint:'start'|'end'):SectionInfrastructure{
 const sections=infrastructureSections(alignment);return (endpoint==='start'?sections[0]:sections.at(-1))?.infrastructure??DEFAULT_INFRASTRUCTURE;
}

/** Screen-space schematic rails. This does not claim lane topology or real track spacing. */
export function schematicTrackPaths(points:{x:number;y:number}[],count:1|2|4):string[]{
 return Array.from({length:count},(_,rail)=>{
  const offset=(rail-(count-1)/2)*3;
  return points.map((point,index)=>{
   const before=points[Math.max(0,index-1)],after=points[Math.min(points.length-1,index+1)];
   const dx=after.x-before.x,dy=after.y-before.y,length=Math.hypot(dx,dy);
   const x=point.x+(length?-dy/length*offset:0),y=point.y+(length?dx/length*offset:0);
   return (index?'L':'M')+x+','+y;
  }).join(' ');
 });
}
