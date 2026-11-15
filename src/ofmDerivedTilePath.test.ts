import {describe,it,expect} from 'vitest';
import {buildTileRoadPath as build} from './ofmDerivedTilePath';
import type {TileRoadLine} from './ofmBoundaryContinuations';
const seam=139.76806640625;
const line=(id:string,x:number,offset=0):TileRoadLine=>({id,revision:'v',z:14,x,y:6451,class:'minor',coordinates:[[seam-.0005,35.684+offset],[seam+.0005,35.684+offset]]});
const start={fragmentId:'a',segmentIndex:0,fraction:0},end={fragmentId:'b',segmentIndex:0,fraction:1};
describe('derived tile path',()=>{
 it('blends a unique pair explicitly and preserves source/picks',()=>{const a=line('a',14552),b=line('b',14553,.000004);const input=JSON.stringify([a,b]);const r=build([a,b],start,end);expect(r.points[0]).toEqual(a.coordinates[0]);expect(r.points.at(-1)).toEqual(b.coordinates[1]);expect(r.geometryDerived).toBe(true);expect(r.joins[0].maxCorrectionMeters).toBeLessThan(.8);expect(r.orderedSpans).toHaveLength(2);expect(JSON.stringify([a,b])).toBe(input);});
 it('reverses route, joins and source direction',()=>{const a=line('a',14552),b=line('b',14553,.000004);const r=build([a,b],end,start);expect(r.points[0]).toEqual(b.coordinates[1]);expect(r.points.at(-1)).toEqual(a.coordinates[0]);expect(r.orderedSpans.map(s=>s.fragmentId)).toEqual(['b','a']);});
 it('builds across three tiles with ordered independent joins',()=>{
  const boundary2=seam+360/16384;
  const a=line('a',14552),b={...line('b',14553),coordinates:[[seam-.0005,35.684],[boundary2+.0005,35.684]] as [number,number][]};
  const c={...line('c',14554),coordinates:[[boundary2-.0005,35.684],[boundary2+.0005,35.684]] as [number,number][]};
  const r=build([a,b,c],start,{...end,fragmentId:'c'});expect(r.joins).toHaveLength(2);expect(r.orderedSpans.map(s=>s.fragmentId)).toEqual(['a','b','c']);
 });
 it('retains original curvature outside the blend',()=>{const a=line('a',14552),b=line('b',14553,.000003);a.coordinates=[[seam-.0005,35.6841],[seam-.0003,35.684],[seam+.0005,35.684]];const r=build([a,b],start,end);expect(r.points[0]).toEqual(a.coordinates[0]);expect(r.points).toContainEqual(a.coordinates[1]);expect(r.joins[0].points.every(p=>p[1]>=35.684&&p[1]<=35.684003)).toBe(true);});
 it('handles a horizontal tile boundary',()=>{const boundary=35.67514743608467;const a={...line('a',14552),coordinates:[[139.755,boundary+.0004],[139.755,boundary-.0004]] as [number,number][]};const b={...a,id:'b',y:6452};const r=build([a,b],start,end);expect(r.joins[0].axis).toBe(1);expect(r.points[0]).toEqual(a.coordinates[0]);});
 it('rejects a pick inside the correction region',()=>expect(()=>build([line('a',14552),line('b',14553)],{...start,fraction:.5},end)).toThrow());
 it('rejects different heights and ambiguous parallel lines',()=>{expect(()=>build([{...line('a',14552),layer:1},line('b',14553)],start,end)).toThrow();expect(()=>build([line('a',14552),line('b',14553),line('c',14553,.000002)],start,end)).toThrow();});
 it('rejects mixed revision and invalid pick',()=>{expect(()=>build([line('a',14552),{...line('b',14553),revision:'other'}],start,end)).toThrow('mixed-revision');expect(()=>build([line('a',14552),line('b',14553)],{...start,fraction:NaN},end)).toThrow('invalid-pick');});
 it('keeps zero offset straight without curvature artifacts',()=>{const r=build([line('a',14552),line('b',14553)],start,end);expect(r.points.every(p=>p[1]===35.684)).toBe(true);expect(r.joins[0].maxCorrectionMeters).toBe(0);});
});
