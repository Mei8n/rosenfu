import {expect,test} from 'vitest';
import {sharedServiceSections,serviceBandPaths,type ServiceMapWorld} from './serviceMapAppearance';
import type {ServicePath} from './servicePaths';
const points=[{latitude:35,longitude:139},{latitude:35,longitude:139.01}];
const leg=(from:string,to:string)=>({key:'1',segment:{kind:'alignment' as const,alignment_id:'a',from,to}});
const path=(id:string,from:string,to:string):ServicePath=>({service_id:id,next_leg_key:'2',legs:[leg(from,to)],stops:[{kind:'start'},{kind:'on_leg',leg_key:'1',station_id:to}]});
const world:ServiceMapWorld={
 services:[{id:'1',start:'s0',name:'紫'},{id:'2',start:'s1',name:'緑'},{id:'3',start:'s3',name:'赤'}],
 alignments:[{id:'a',points}],
 station_placements:[0,.25,.75,1].map((position,i)=>({station_id:'s'+i,alignment_id:'a',position})),
 service_paths:[path('1','s0','s3'),path('2','s1','s2'),path('3','s3','s1')],
 service_appearances:[{service_id:'1',color:'#800080'},{service_id:'2',color:'#008000'},{service_id:'3',color:'#ff0000'}]
};
test('splits only overlapping portions into equal bands including reverse services',()=>{
 const sections=sharedServiceSections(world);
 expect(sections.map(s=>[s.begin,s.finish,s.bands.map(b=>b.serviceId)])).toEqual([[0,.25,['1']],[.25,.75,['1','2','3']],[.75,1,['1','3']]]);
 expect(sections[1].bands.map(b=>b.color)).toEqual(['#800080','#008000','#ff0000']);
 const bands=serviceBandPaths([{x:0,y:0},{x:100,y:0}],sections[1].bands);
 expect(bands.reduce((n,b)=>n+b.width,0)).toBe(8);
 expect(new Set(bands.map(b=>b.d)).size).toBe(3);
});
test('repeated traversal by one service never doubles its band',()=>{
 const p=path('1','s0','s3');p.legs.push({...leg('s3','s0'),key:'2'});
 const result=sharedServiceSections({...world,services:[world.services[0]],service_paths:[p]});
 expect(result).toHaveLength(1);expect(result[0].bands).toHaveLength(1);
});
test('invalid physical path does not paint unrelated or partial infrastructure',()=>{
 const p=path('1','s0','s3');p.legs.push({...leg('missing','s0'),key:'2'});
 expect(sharedServiceSections({...world,services:[world.services[0]],service_paths:[p]})).toEqual([]);
 expect(sharedServiceSections(null)).toEqual([]);
});
test('span rendering never mutates saved route or stopping data',()=>{
 const before=JSON.stringify(world);sharedServiceSections(world);expect(JSON.stringify(world)).toBe(before);
});
test('connected physical corridor crosses alignments without fabricating nearby tracks',()=>{
 const connection={a:{alignment_id:'a',endpoint:'end' as const},b:{alignment_id:'b',endpoint:'start' as const}};
 const w:ServiceMapWorld={...world,alignments:[...world.alignments!,{id:'b',points:points.map(p=>({...p,longitude:p.longitude+.01}))}],services:[{id:'1',start:'s1',name:'直通'}],alignment_connections:[connection],station_placements:[...world.station_placements!,{station_id:'end',alignment_id:'b',position:.8}],service_paths:[{service_id:'1',next_leg_key:'2',legs:[{key:'1',segment:{kind:'connected_alignments',from:'s1',to:'end',connections:[connection]}}],stops:[]}]};
 expect(sharedServiceSections(w).map(s=>[s.alignmentId,s.begin,s.finish])).toEqual([['a',.25,1],['b',0,.8]]);
 expect(sharedServiceSections({...w,alignment_connections:[]})).toEqual([]);
});
