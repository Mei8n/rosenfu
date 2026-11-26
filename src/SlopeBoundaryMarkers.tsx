import {useMemo} from 'react';
import {project,viewBounds,type Camera} from './map';
import {createTrackViewportIndex,intersectsTrackView,trackBounds} from './trackViewport';
import type {GeoPoint} from './alignment';
import type {HeightAlignment} from './BuiltAnchorHeight';
import type {SectionInfrastructure} from './sectionInfrastructure';
import {anchorHeights} from './anchorHeights';

type Boundary={point:GeoPoint;height:number;kind:'start'|'end';planned:boolean};
type Planned={points:GeoPoint[];anchorIndices?:number[];anchorLevels?:SectionInfrastructure['level'][];startLevel?:SectionInfrastructure['level'];infrastructure:SectionInfrastructure};
const levels:SectionInfrastructure['level'][]=['underground_3','underground_2','underground','ground','elevated','elevated_2','elevated_3'];
const metres=(level:SectionInfrastructure['level'])=>(levels.indexOf(level)-3)*6;
/** Profiles, rather than infrastructure labels, determine the actual ramp ends.
 * A clipped Hermite profile can be nonflat even when its two heights agree. */
export function builtSlopeBoundaries(lines:HeightAlignment[]):Boundary[]{
 const result:Boundary[]=[];
 for(const line of lines){
  const indices=line.anchor_indices??[0,line.points.length-1];
  const profiles=line.section_profiles?.length?line.section_profiles:line.vertical_profile?[line.vertical_profile]:[];
  const samples=line.section_profiles?.length?indices:[0,line.points.length-1];
  profiles.forEach((profile,i)=>{
   if(profile.start_height_m===profile.end_height_m&&!(profile.start_tangent_m??0)&&!(profile.end_tangent_m??0))return;
   const a=line.points[samples[i]],b=line.points[samples[i+1]];
   if(!a||!b)return;
   result.push({point:a,height:profile.start_height_m,kind:'start',planned:false},{point:b,height:profile.end_height_m,kind:'end',planned:false});
  });
 }
 return result;
}
export function plannedSlopeBoundaries(plans:Planned[]):Boundary[]{
 return plans.flatMap(plan=>{
  const indices=plan.anchorIndices??[0,plan.points.length-1];
  if(plan.points.length<2||plan.anchorLevels&&plan.anchorLevels.length!==indices.length)return [];
  const heights=anchorHeights(indices.length,plan.anchorLevels,plan.startLevel,plan.infrastructure.level).map(metres);
  const result:Boundary[]=[];
  for(let i=0;i+1<indices.length;i++){
   if(heights[i]===heights[i+1])continue;
   const a=plan.points[indices[i]],b=plan.points[indices[i+1]];
   if(a&&b)result.push({point:a,height:heights[i],kind:'start',planned:true},{point:b,height:heights[i+1],kind:'end',planned:true});
  }
  return result;
 });
}
function distinctBoundaries(marks:Boundary[]){
 const seen=new Set<string>();
 return marks.flatMap((mark,index)=>{
  const key=JSON.stringify([mark.point,mark.height]);if(seen.has(key))return [];seen.add(key);
  return [{...mark,index,points:[mark.point]}];
 });
}
/** Screen-sized white-backed marks remain legible on map/track backgrounds.
 * User requested both boundaries (TW uses middle-of-ramp directional marks). */
export function SlopeBoundaryMarkers({lines,plans,camera,scale,mapSize}:{lines:HeightAlignment[];plans:Planned[];camera:Camera;scale:number;mapSize:{width:number;height:number}}){
 const builtIndex=useMemo(()=>createTrackViewportIndex(distinctBoundaries(builtSlopeBoundaries(lines)),'display'),[lines]);
 const planned=useMemo(()=>distinctBoundaries(plannedSlopeBoundaries(plans)),[plans]);
 // These fixed-size numerical labels may extend beyond an off-screen point.
 // Keep a generous screen-space guard band for text, its outline and markers.
 const view=useMemo(()=>viewBounds(camera,mapSize,384/scale),[camera,mapSize,scale]);
 const marks=useMemo(()=>[...builtIndex.visible(view),...planned.filter(mark=>intersectsTrackView(trackBounds(mark.points),view))],[builtIndex,planned,view]);
 return <g pointerEvents="none" className="slope-boundary-markers">{marks.map(mark=>{
 const p=project(mark.point.latitude,mark.point.longitude,camera),height=Number(mark.height.toFixed(1));
  return <g key={`${mark.planned}:${mark.index}`} data-slope-boundary={mark.kind} data-planned={mark.planned} transform={`translate(${p.x} ${p.y}) scale(${1/scale})`} aria-label={`スロープ端点 ${height} m`}>
   <path d="M -6 5 L 0 -5 L 6 5 Z" fill="white" stroke={mark.planned?'#1579b8':'#724c22'} strokeWidth="2" strokeDasharray={mark.planned?'2 1':undefined}/>
   <text x="9" y="-6" fontSize="11" fontWeight="600" fill={mark.planned?'#1579b8':'#724c22'} stroke="white" strokeWidth="3" paintOrder="stroke">{height>0?'+':''}{height} m</text>
  </g>;
 })}</g>;
}
