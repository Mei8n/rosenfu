import {describe,it,expect} from 'vitest';
import {twoEndedJoin} from './twoEndedJoin';
import {plannedGeometry} from './editedGeometry';
import {extensionJoinError} from './extensionJoin';
import {anchorIndices,deletionSections} from './deletionSections';
const p=(x:number,y=0)=>({latitude:35+y/111195,longitude:135+x/(111195*Math.cos(35*Math.PI/180))});
describe('two-ended endpoint fitting',()=>{
 it('fits an S connection with both existing headings preserved',()=>{
  const a=p(0),b=p(300,100),before=p(-30),inside=p(330,100);
  const fit=twoEndedJoin([a,b],undefined,before,inside)!;
  expect(fit).toBeDefined();expect(fit.line[0]).toEqual(a);expect(fit.line.at(-1)).toEqual(b);
  expect(extensionJoinError(fit.line,before)).toBeNull();expect(extensionJoinError([...fit.line].reverse(),inside)).toBeNull();
  expect(plannedGeometry([a,b],JSON.parse(JSON.stringify(fit)),before).points).toEqual(fit.line);
 });
 it('preserves earlier spans and supports undo truncation',()=>{
  const anchors=[p(0),p(200),p(500,100)],fit=twoEndedJoin(anchors,undefined,p(-30),p(530,100))!;
  expect(fit).toBeDefined();expect(fit.line.slice(0,2)).toEqual(anchors.slice(0,2));
  expect(plannedGeometry(anchors.slice(0,2),fit,p(-30)).points).toEqual(anchors.slice(0,2));
 });
 it('preserves an edited prefix, and redo restores the identical fitted line',()=>{
  const anchors=[p(0),p(200),p(500,100)],prefix={version:1 as const,anchors:anchors.slice(0,2),line:[p(0),p(100),p(200)]};
  const fit=twoEndedJoin(anchors,prefix,p(-30),p(530,100))!;
  expect(fit.line.slice(0,3)).toEqual(prefix.line);
  const indices=anchorIndices(anchors,fit.line);
  expect(indices).toEqual([0,2,fit.line.length-1]);
  const restored=JSON.parse(JSON.stringify({points:fit.line,anchor_indices:indices}));
  expect(deletionSections(restored).map(section=>[section.points[0],section.points.at(-1)])).toEqual([[anchors[0],anchors[1]],[anchors[1],anchors[2]]]);
  expect(plannedGeometry(anchors.slice(0,2),fit,p(-30)).points).toEqual(prefix.line);
  expect(plannedGeometry(anchors,fit,p(-30)).points).toEqual(fit.line);
 });
 it('rejects invalid coordinates without producing a persisted shape',()=>{
  expect(twoEndedJoin([p(0),{latitude:NaN,longitude:135}],undefined,p(-30),p(330))).toBeUndefined();
 });
 it('rejects reverse-facing joins and too tight curves',()=>{
  expect(twoEndedJoin([p(0),p(300)],undefined,p(-30),p(270))).toBeUndefined();
  expect(twoEndedJoin([p(0),p(5,5)],undefined,p(-30),p(10,5))).toBeUndefined();
 });
});
