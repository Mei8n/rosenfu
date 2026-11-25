// @vitest-environment jsdom
import {cleanup,render} from '@testing-library/react';
import {expect,test} from 'vitest';
import {TrackGeometryLayer,type TrackRenderGeometry} from './TrackRenderGeometry';
import {DEFAULT_CAMERA,project} from './map';

test.skipIf((import.meta as ImportMeta & {env?:Record<string,string>}).env?.VITE_ROSENFU_TRACK_TIMING!=='1')('times the actual physical-track layer on a dispersed synthetic world',()=>{
 const geometry:TrackRenderGeometry={lanes:[],movements:[]};
 const lines:{id:string}[]=[];
 for(let i=0;i<40000;i++){
  const id=String(i);
  // 32 lanes remain inside every camera in the pan series.
  const latitude=i<32?35.67504+(i%8)*.0002:37+(i%103)*.055;
  const longitude=i<32?139.75+(i%4)*.0002:127+(i%149)*.105;
  lines.push({id});
  geometry.lanes.push({alignment_id:id,lane_index:0,section_index:0,points:[{latitude,longitude},{latitude,longitude:longitude+.001}]});
 }
 const view=(i:number)=><svg><TrackGeometryLayer geometry={geometry} camera={{...DEFAULT_CAMERA,longitude:139.75+(i%31)*.0001}} lines={lines} selectedId=""/></svg>;
 const times:number[]=[],mountTimes:number[]=[];
 for(let run=0;run<5;run++){
  const start=performance.now(),{container,rerender}=render(view(0));
  const mount=performance.now()-start;
  for(let i=0;i<20;i++)rerender(view(i));
  for(let i=0;i<120;i++){
   const t=performance.now();
   rerender(view(i));
   if(run>0)times.push(performance.now()-t);
  }
  if(run>0)mountTimes.push(mount);
  const nodes=[...container.querySelectorAll('[data-physical-alignment]')];
  expect(nodes.map(n=>n.getAttribute('data-physical-alignment'))).toEqual(Array.from({length:32},(_,i)=>String(i)));
  const camera={...DEFAULT_CAMERA,longitude:139.75+(119%31)*.0001};
  const expectedPaths=geometry.lanes.slice(0,32).map(lane=>lane.points.map((pt,i)=>{
   const {x,y}=project(pt.latitude,pt.longitude,camera);
   return `${i?'L':'M'}${x},${y}`;
  }).join(' '));
  expect(nodes.map(node=>node.getAttribute('d'))).toEqual(expectedPaths);
  cleanup();
 }
 const summary=(samples:number[])=>{
  const sorted=[...samples].sort((a,b)=>a-b);
  return {n:sorted.length,p50Ms:sorted[Math.floor(.5*sorted.length)],p95Ms:sorted[Math.floor(.95*sorted.length)]};
 };
 console.log('Track viewport timing',JSON.stringify({mount:summary(mountTimes),pan:summary(times)}));
});
