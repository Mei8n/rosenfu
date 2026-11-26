import {expect,test} from 'vitest';
import {mapRenderMetrics} from './mapRenderMetrics';
import {serviceBandPaths} from './serviceMapAppearance';
test('service assignment adds only modest weight at every zoom and width setting',()=>{
 for(const zoom of [5,9,10.5,12,15,19])for(const width of [2,4,8]){
  const m=mapRenderMetrics(zoom,width,1);
  expect(m.service/m.track).toBeCloseTo(1.15);
  expect(m.stationRadius*2).toBeGreaterThan(m.service);
 }
});
test('screen dimensions and stripe offsets remain stable across viewport sizes',()=>{
 const bands=[{serviceId:'1',color:'#f00',name:'A'},{serviceId:'2',color:'#00f',name:'B'}];
 const baseline=mapRenderMetrics(12,4,1);
 for(const scale of [.5,1,2]){
  const m=mapRenderMetrics(12,4,scale);
  const paths=serviceBandPaths([{x:0,y:0},{x:100,y:0}],bands,m.service);
  expect(paths.reduce((sum,p)=>sum+p.width,0)*scale).toBeCloseTo(baseline.service);
  expect(m.serviceCasing*scale).toBeCloseTo(baseline.serviceCasing);
  expect(m.stationRadius*scale).toBeCloseTo(baseline.stationRadius);
  expect(m.stationLabelSize*scale).toBe(12);
 }
});
test('zoom response is continuous and bounded rather than fixed thick at overview scale',()=>{
 expect(mapRenderMetrics(9,4,1).track).toBeLessThan(mapRenderMetrics(15,4,1).track);
 expect(mapRenderMetrics(12.001,4,1).track-mapRenderMetrics(12,4,1).track).toBeLessThan(.001);
 for(const value of Object.values(mapRenderMetrics(NaN,NaN,0)))expect(Number.isFinite(value)).toBe(true);
});
