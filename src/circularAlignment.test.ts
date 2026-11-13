import {expect,test} from 'vitest';
import {circularAlignment} from './circularAlignment';
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
