import {memo,useMemo} from 'react';
import {TrackStroke} from './TrackStroke';
import {project,type Camera} from './map';
import type {GeoPoint} from './alignment';
import {DEFAULT_INFRASTRUCTURE,schematicTrackPaths,type SectionInfrastructure} from './sectionInfrastructure';

/** One planned stroke, placed between the existing hit targets and anchors.
 * Keeping that placement preserves SVG event ancestry and stacking order. */
export const ConstructionLayer=memo(function ConstructionLayer({points,camera,infrastructure=DEFAULT_INFRASTRUCTURE,className,pathId}:{points:GeoPoint[];camera:Camera;infrastructure?:SectionInfrastructure;className:string;pathId?:number}){
 const d=useMemo(()=>schematicTrackPaths(points.map(point=>project(point.latitude,point.longitude,camera)),infrastructure.track_count).join(' '),[points,camera,infrastructure.track_count]);
 return <TrackStroke d={d} data-track-count={infrastructure.track_count} className={className} data-plan-id={pathId} pointerEvents="none"/>;
});
