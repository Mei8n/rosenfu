import {expect,test} from 'vitest';
import {serviceRenderChains,serviceBandPaths,type SharedServiceSection} from './serviceMapAppearance';
const p=(x:number,y=0)=>({latitude:35+y*.001,longitude:139+x*.001});
const bands=[{serviceId:'s1',color:'#123456',name:'普通'},{serviceId:'s2',color:'#654321',name:'快速'}];
const section=(id:string,points:ReturnType<typeof p>[],extra:Partial<SharedServiceSection>={}):SharedServiceSection=>({alignmentId:id,begin:0,finish:1,points,bands,...extra});
const connection=(a:string,b:string,ae:'start'|'end'='end',be:'start'|'end'='start')=>({a:{alignment_id:a,endpoint:ae},b:{alignment_id:b,endpoint:be}});
test('separately built curved pieces use one stripe offset and match a single continuous path',()=>{
 const points=[p(0),p(1),p(2,1),p(3,1)];
 const sections=[section('a',points.slice(0,2)),section('b',points.slice(1))];
 const before=JSON.stringify(sections);
 const chains=serviceRenderChains(sections,[connection('a','b')]);
 expect(chains).toHaveLength(1);expect(chains[0].points).toEqual(points);
 const project=(ps:ReturnType<typeof p>[])=>ps.map(q=>({x:q.longitude*10000,y:q.latitude*10000}));
 expect(serviceBandPaths(project(chains[0].points),bands)).toEqual(serviceBandPaths(project(points),bands));
 expect(JSON.stringify(sections)).toBe(before);
});
test('reversed storage directions and section ordering still form one chain',()=>{
 const sections=[section('b',[p(2,1),p(1)]),section('a',[p(0),p(1)])];
 const chains=serviceRenderChains(sections,[connection('a','b','end','end')]);
 expect(chains).toHaveLength(1);expect(chains[0].points).toEqual([p(2,1),p(1),p(0)]);
});
test('crossing or nearby geometry without a connection is never combined',()=>{
 expect(serviceRenderChains([section('a',[p(0),p(1)]),section('b',[p(1),p(2)])],[])).toHaveLength(2);
 expect(serviceRenderChains([section('a',[p(0),p(2,2)]),section('b',[p(0,2),p(2)])],[])).toHaveLength(2);
});
test('branches, differing service sets, partial spans and stale endpoint positions remain separate',()=>{
 const a=section('a',[p(0),p(1)]),b=section('b',[p(1),p(2)]),c=section('c',[p(1),p(2,1)]);
 expect(serviceRenderChains([a,b,c],[connection('a','b'),connection('a','c')])).toHaveLength(3);
 expect(serviceRenderChains([a,{...b,bands:[bands[0]]}],[connection('a','b')])).toHaveLength(2);
 expect(serviceRenderChains([{...a,finish:.9},b],[connection('a','b')])).toHaveLength(2);
 expect(serviceRenderChains([a,{...b,points:[p(1.1),p(2)]}],[connection('a','b')])).toHaveLength(2);
});
test('closed network terminates and retains a closed colour chain',()=>{
 const chains=serviceRenderChains([section('a',[p(0),p(1)]),section('b',[p(1),p(1,1)]),section('c',[p(1,1),p(0)])],[connection('a','b'),connection('b','c'),connection('c','a')]);
 expect(chains).toHaveLength(1);expect(chains[0].points).toEqual([p(0),p(1),p(1,1),p(0)]);
});

test('closed curved colour bands meet exactly at the cycle seam',()=>{
 const points=[{x:0,y:0},{x:100,y:0},{x:100,y:100},{x:0,y:0}];
 for(const band of serviceBandPaths(points,bands)){
  const commands=band.d.split(' ');
  expect(commands[0].slice(1)).toBe(commands.at(-1)!.slice(1));
 }
});
test('rounding differences at a saved closed connection still close the rendered band',()=>{
 const chains=serviceRenderChains([section('a',[p(0),p(1)]),section('b',[p(1),p(1,1)]),section('c',[p(1,1),p(.000001)])],[connection('a','b'),connection('b','c'),connection('c','a')]);
 expect(chains).toHaveLength(1);expect(chains[0].points.at(-1)).toEqual(chains[0].points[0]);
});
