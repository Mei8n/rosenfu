import {expect,test} from 'vitest';
import {circularAlignment,provisionalCurveSpeed} from './circularAlignment';
import {project,DEFAULT_CAMERA} from './map';
const p=(x:number,y:number)=>({longitude:139.75+x/90000,latitude:35.67504+y/111000});
test('first chord is straight; later segment follows one true display circle',()=>{
 const a=[p(0,0),p(100,0),p(200,100)],r=circularAlignment(a);
 expect(r.error).toBeNull();expect(r.points[0]).toEqual(a[0]);expect(r.points.at(-1)).toEqual(a[2]);
 const g=r.guide!,c=project(g.center.latitude,g.center.longitude),e=project(g.edge.latitude,g.edge.longitude);
 const radius=Math.hypot(e.x-c.x,e.y-c.y);
 for(const v of r.points.slice(1)){const s=project(v.latitude,v.longitude,DEFAULT_CAMERA);expect(Math.hypot(s.x-c.x,s.y-c.y)).toBeCloseTo(radius,7);}
});
test('append, undo and redo preserve all earlier sampled segments',()=>{
 const a=[p(0,0),p(100,0),p(200,100)],r=circularAlignment(a),more=circularAlignment([...a,p(100,200)]);
 expect(more.error).toBeNull();expect(more.points.slice(0,r.points.length)).toEqual(r.points);
 expect(circularAlignment(a)).toEqual(r);
});
test('right-hand curve, straight continuation and empty are finite',()=>{
 expect(circularAlignment([]).points).toEqual([]);
 const r=circularAlignment([p(0,0),p(100,0),p(200,-100)]);expect(r.error).toBeNull();expect(r.guide).not.toBeNull();
 expect(circularAlignment([p(0,0),p(100,0),p(200,0)]).guide).toBeNull();
});
test('duplicate, backward and too tight curves explain refusal',()=>{
 expect(circularAlignment([p(0,0),p(0,0)]).error).toMatch(/重複/);
 expect(circularAlignment([p(0,0),p(100,0),p(80,20)]).error).toMatch(/折り返/);
 expect(circularAlignment([p(0,0),p(100,0),p(110,10)]).error).toMatch(/25m/);
 expect(circularAlignment([{latitude:NaN,longitude:0}]).error).toMatch(/座標/);
});

test('arc metadata identifies each sampled segment and analytical midpoint',()=>{
 const a=[p(0,0),p(100,0),p(200,100),p(100,200)],r=circularAlignment(a);
 expect(r.arcs).toHaveLength(2);
 expect(r.arcs.map(arc=>arc.segmentIndex)).toEqual([1,2]);
 for(const arc of r.arcs){
  expect(r.points[arc.startPointIndex]).toEqual(a[arc.segmentIndex]);
  expect(r.points[arc.endPointIndex]).toEqual(a[arc.segmentIndex+1]);
  expect(Number.isFinite(arc.midpoint.latitude)&&Number.isFinite(arc.midpoint.longitude)).toBe(true);
  expect(arc.radiusM).toBeGreaterThan(25);
 }
 const one=circularAlignment(a.slice(0,3));
 expect(r.arcs[0]).toEqual(one.arcs[0]);
 const g=one.guide!,c=project(g.center.latitude,g.center.longitude),edge=project(g.edge.latitude,g.edge.longitude),mid=project(one.arcs[0].midpoint.latitude,one.arcs[0].midpoint.longitude);
 expect(Math.hypot(mid.x-c.x,mid.y-c.y)).toBeCloseTo(Math.hypot(edge.x-c.x,edge.y-c.y),7);
});
test('right-hand arc has midpoint; straight and refused backward segments add no arcs',()=>{
 expect(circularAlignment([p(0,0),p(100,0),p(200,-100)]).arcs).toHaveLength(1);
 expect(circularAlignment([p(0,0),p(100,0),p(200,0)]).arcs).toEqual([]);
 expect(circularAlignment([p(0,0),p(100,0),p(80,20)]).arcs).toEqual([]);
 const tight=circularAlignment([p(0,0),p(100,0),p(110,10)]);
 expect(tight.error).toMatch(/25m/);expect(tight.arcs).toHaveLength(1);
});
test('provisional curve speeds use explicit game model without fake straight speeds',()=>{
 expect([25,300,1000,2000,8000].map(provisionalCurveSpeed)).toEqual([18,62,113,160,321]);
 for(const radius of [0,-1,NaN,Infinity,-Infinity])expect(provisionalCurveSpeed(radius)).toBeNull();
});

test('extension starts with the existing endpoint tangent rather than a new chord',()=>{
 const previous={latitude:35,longitude:139},start={latitude:35,longitude:139.01},end={latitude:35.003,longitude:139.02};
 const result=circularAlignment([start,end],previous);
 expect(result.error).toBeNull();expect(result.points.length).toBeGreaterThan(2);
 const first=result.points[1];const turn=Math.atan2(first.latitude-start.latitude,(first.longitude-start.longitude)*Math.cos(35*Math.PI/180));
 expect(Math.abs(turn)).toBeLessThan(5*Math.PI/180);
 expect(circularAlignment([start,previous],previous).error).toMatch(/折り返し/);
});
