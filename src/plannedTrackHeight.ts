import {nearestOnAlignment,type GeoPoint} from './alignment';
import {anchorHeights} from './anchorHeights';
import {anchorIndices} from './deletionSections';
import {DEFAULT_INFRASTRUCTURE,type SectionInfrastructure} from './sectionInfrastructure';
type Level=SectionInfrastructure['level'];
const METRES:Record<Level,number>={ground:0,elevated:6,elevated_2:12,elevated_3:18,underground:-6,underground_2:-12,underground_3:-18};
const metres=(level:Level)=>METRES[level]>0?`+${METRES[level]}`:String(METRES[level]);
/** Report section endpoints, not a guessed interpolation of the native vertical profile. */
export function plannedTrackHeight(path:{points:GeoPoint[];anchorLevels?:Level[];startLevel?:Level;infrastructure?:SectionInfrastructure},line:GeoPoint[],target:GeoPoint):string{
 const boundaries=anchorIndices(path.points,line);
 if(!boundaries||path.anchorLevels&&path.anchorLevels.length!==path.points.length)return '高さ：未確認';
 const levels=anchorHeights(path.points.length,path.anchorLevels,path.startLevel,(path.infrastructure??DEFAULT_INFRASTRUCTURE).level);
 if(levels.some(level=>METRES[level]===undefined))return '高さ：未確認';
 let section=0,distance=Infinity;
 for(let i=0;i<boundaries.length-1;i++){
  const candidate=nearestOnAlignment(line.slice(boundaries[i],boundaries[i+1]+1),target).distanceM;
  if(candidate<distance){distance=candidate;section=i;}
 }
 const start=levels[section],end=levels[section+1];
 return start===end?`高さ ${metres(start)} m`:`区間端点高 ${metres(start)} → ${metres(end)} m`;
}
