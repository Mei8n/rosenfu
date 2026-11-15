import {HeightControls} from './HeightControls';
import {alignmentLength} from './alignment';
import {project,type Camera} from './map';
import type {GeoPoint} from './constructionPlans';
import type {SectionInfrastructure} from './sectionInfrastructure';
type Profile={start_height_m:number;end_height_m:number;start_tangent_m?:number;end_tangent_m?:number};
export type HeightAlignment={id:string;owner:string;points:GeoPoint[];anchor_indices?:number[]|null;infrastructure?:SectionInfrastructure[];section_profiles?:Profile[];vertical_profile?:Profile|null};
export type BuiltHeightSelection={alignment:HeightAlignment;anchorIndex:number};
const levels:SectionInfrastructure['level'][]=['underground_3','underground_2','underground','ground','elevated','elevated_2','elevated_3'];
export function builtAnchorHeight(line:HeightAlignment,index:number):number{
 const indices=line.anchor_indices??[0,line.points.length-1];
 if(index<0||index>=indices.length)return NaN;
 if(line.section_profiles?.length)return index===0?line.section_profiles[0].start_height_m:line.section_profiles[index-1].end_height_m;
 if(line.vertical_profile){const p=line.vertical_profile,t=alignmentLength(line.points.slice(0,indices[index]+1))/alignmentLength(line.points);return (2*t*t*t-3*t*t+1)*p.start_height_m+(t*t*t-2*t*t+t)*(p.start_tangent_m??0)+(-2*t*t*t+3*t*t)*p.end_height_m+(t*t*t-t*t)*(p.end_tangent_m??0);}
 return (levels.indexOf(line.infrastructure?.[Math.min(index,indices.length-2)]?.level??'ground')-3)*6;
}
export function BuiltAnchorHeightToolbar({selection,disabled,onChange}:{selection:BuiltHeightSelection;disabled:boolean;onChange:(level:SectionInfrastructure['level'])=>void}){
 const height=builtAnchorHeight(selection.alignment,selection.anchorIndex),level=levels.find((_,index)=>Math.abs((index-3)*6-height)<1e-6);
 return <div className="construction-toolbar" role="group" aria-label="建設済みアンカーの高さ"><span>建設済み · 高さ {Number.isFinite(height)?`${Number(height.toFixed(2))} m`:'未取得'}</span><HeightControls value={level} disabled={disabled||!Number.isFinite(height)} onChange={onChange}/></div>;
}
export function BuiltAnchorHeightHandles({lines,owner,camera,scale,onSelect}:{lines:HeightAlignment[];owner:string|undefined;camera:Camera;scale:number;onSelect:(selection:BuiltHeightSelection)=>void}){
 return <g className="built-anchor-height-handles">{lines.filter(line=>line.owner===owner).flatMap(line=>(line.anchor_indices??[0,line.points.length-1]).map((sample,index)=>{
 const p=project(line.points[sample].latitude,line.points[sample].longitude,camera);
 return <circle key={`${line.id}-${index}`} data-built-height-anchor={index} data-built-height-alignment={line.id} cx={p.x} cy={p.y} r={5/scale} fill="white" stroke="#566d63" strokeWidth={1.5/scale} role="button" tabIndex={0} aria-label={`建設済みアンカー ${index+1} の高さを変更`} onPointerDown={e=>e.stopPropagation()} onClick={e=>{e.stopPropagation();onSelect({alignment:line,anchorIndex:index});}} onKeyDown={e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();e.stopPropagation();onSelect({alignment:line,anchorIndex:index});}}}/>;
 }))}</g>;
}
