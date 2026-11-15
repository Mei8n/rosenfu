import {describe,it,expect} from 'vitest';
import {parallelProjection,offsetTrack,parallelSection} from './parallelTrack';
import {plannedGeometry} from './editedGeometry';
const line=[{latitude:35,longitude:139},{latitude:35.002,longitude:139},{latitude:35.004,longitude:139}];
describe('parallel built tracks',()=>{
 it('offsets either side at a fixed distance',()=>{for(const offset of [-10,10]){const result=offsetTrack(line,offset)!;expect(result).toHaveLength(3);expect(parallelProjection(line,result[1])!.offset).toBeCloseTo(offset,2);}});
 it('slices in either direction and preserves sampled geometry through undo and restore',()=>{const offset=offsetTrack(line,10)!;for(const points of [[offset[0],offset[2]],[offset[2],offset[0]]]){const g=parallelSection(offset,points[0],points[1])!;expect(g.line).toHaveLength(3);expect(plannedGeometry(g.anchors,JSON.parse(JSON.stringify(g))).points).toEqual(g.line);expect(plannedGeometry(g.anchors.slice(0,1),g).points).toEqual([g.anchors[0]]);}});
 it('rejects unsafe offsets, short sections and reversals',()=>{expect(offsetTrack(line,0)).toBeNull();expect(offsetTrack(line,31)).toBeNull();expect(parallelSection(line,line[0],line[0])).toBeNull();expect(offsetTrack([line[0],line[1],line[0]],10)).toBeNull();});
});

it('preserves curved offset radius and direction on both sides',()=>{
 const radius=120,center={latitude:35,longitude:139},k=111195,c=Math.cos(35*Math.PI/180);
 const arc=Array.from({length:31},(_,i)=>{const t=i*Math.PI/60;return {latitude:center.latitude+radius*Math.sin(t)/k,longitude:center.longitude+radius*Math.cos(t)/(k*c)};});
 for(const offset of [-10,10]){const curve=offsetTrack(arc,offset)!;expect(curve).not.toBeNull();expect(curve.length).toBe(arc.length);const section=parallelSection(curve,curve.at(-1)!,curve[0])!;expect(section.line).toEqual([...curve].reverse());expect(plannedGeometry(section.anchors,section).error).toBeNull();}
});
