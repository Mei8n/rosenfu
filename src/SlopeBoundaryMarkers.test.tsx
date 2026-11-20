import {describe,it,expect} from 'vitest';
import {renderToStaticMarkup} from 'react-dom/server';
import {builtSlopeBoundaries,plannedSlopeBoundaries,SlopeBoundaryMarkers} from './SlopeBoundaryMarkers';
import {DEFAULT_CAMERA} from './map';
const points=[{latitude:35,longitude:139},{latitude:35.01,longitude:139},{latitude:35.02,longitude:139}];
describe('slope boundaries',()=>{
 it('only marks nonflat profile sections at actual samples',()=>{
  const marks=builtSlopeBoundaries([{id:'1',owner:'1',points,anchor_indices:[0,1,2],section_profiles:[{start_height_m:0,end_height_m:6},{start_height_m:6,end_height_m:6}]}]);
  expect(marks.map(m=>[m.point,m.height])).toEqual([[points[0],0],[points[1],6]]);
 });
 it('handles clipped nonflat equal-height profiles and whole-line profiles',()=>{
  expect(builtSlopeBoundaries([{id:'1',owner:'1',points,vertical_profile:{start_height_m:0,end_height_m:0,start_tangent_m:6}}]).map(m=>m.point)).toEqual([points[0],points[2]]);
 });
 it('shows both boundaries for rising and falling planned sections',()=>{
  const marks=plannedSlopeBoundaries([{points,anchorIndices:[0,1,2],anchorLevels:['ground','elevated','underground'],infrastructure:{track_count:2,level:'ground'}}]);
  expect(marks.map(m=>m.height)).toEqual([0,6,6,-6]);
 });
 it('does not mark flat plans or stale height metadata',()=>{
  expect(plannedSlopeBoundaries([{points,infrastructure:{track_count:1,level:'ground'}}])).toEqual([]);
  expect(plannedSlopeBoundaries([{points,anchorLevels:['ground'],infrastructure:{track_count:1,level:'ground'}}])).toEqual([]);
 });
 it('uses distinct planned styling without pointer interception or native title tooltip',()=>{
  const html=renderToStaticMarkup(<SlopeBoundaryMarkers lines={[]} plans={[{points,startLevel:'ground',infrastructure:{track_count:1,level:'elevated'}}]} camera={DEFAULT_CAMERA} scale={2}/>);
  expect(html).toContain('pointer-events="none"');expect(html).toContain('+6 m');expect(html).toContain('scale(0.5)');expect(html).not.toContain('<title');
 });
});
