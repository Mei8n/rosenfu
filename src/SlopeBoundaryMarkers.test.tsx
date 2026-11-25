// @vitest-environment jsdom
import {describe,it,expect} from 'vitest';
import {renderToStaticMarkup} from 'react-dom/server';
import {builtSlopeBoundaries,plannedSlopeBoundaries,SlopeBoundaryMarkers} from './SlopeBoundaryMarkers';
import {DEFAULT_CAMERA,MAP_VIEWBOX,cameraAt,panCamera,unproject} from './map';
import {cleanup,render} from '@testing-library/react';
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
  const camera=cameraAt(points[1].latitude,points[1].longitude);
  const html=renderToStaticMarkup(<SlopeBoundaryMarkers lines={[]} plans={[{points,startLevel:'ground',infrastructure:{track_count:1,level:'elevated'}}]} camera={camera} scale={2} mapSize={{width:2000,height:1200}}/>);
  expect(html).toContain('pointer-events="none"');expect(html).toContain('+6 m');expect(html).toContain('scale(0.5)');expect(html).not.toContain('<title');
 });
});


it('culls distant slope marks, keeps edge labels, and reuses the built index on pan',()=>{
 let calls=0;
 const height=(end:number)=>({get start_height_m(){calls++;return 0;},end_height_m:end});
 const line=(id:string,points:typeof local,end:number)=>({id,owner:'1',points,vertical_profile:height(end)});
 const camera=DEFAULT_CAMERA;
 const local=[unproject(500,300,camera),unproject(515,305,camera)];
 const edge=[unproject(0,300,camera),unproject(0,310,camera)];
 const distant=[{latitude:43.1,longitude:141.3},{latitude:43.11,longitude:141.3}];
 const lines=[line('local',local,6),line('edge',edge,12),
  ...Array.from({length:600},(_,i)=>line('far-'+i,distant.map(p=>({...p,longitude:p.longitude+i*.00001})),18)),
 ];
 const view=(camera:typeof DEFAULT_CAMERA)=><svg><SlopeBoundaryMarkers lines={lines} plans={[]} camera={camera} scale={1} mapSize={MAP_VIEWBOX}/></svg>;
 const rendered=render(view(camera));
 try{
  const marks=()=>[...rendered.container.querySelectorAll('[data-slope-boundary]')];
  expect(marks()).toHaveLength(4);
  const old=marks()[3];
  const builtReads=calls;
  expect(builtReads).toBeGreaterThan(600);
  rendered.rerender(view(panCamera(camera,8,0)));
  expect(marks()).toHaveLength(4);
  expect(marks()[3]).toBe(old);
  expect(old.getAttribute('transform')).toMatch(/^translate\(-/);
  expect(old.getAttribute('aria-label')).toBe('スロープ端点 12 m');
  expect(calls).toBe(builtReads);
  rendered.rerender(view(cameraAt(43.1,141.3)));
  expect(marks()).toHaveLength(1200);
  expect(calls).toBe(builtReads);
 }finally{cleanup();}
});
