import {expect,it} from 'vitest';
import {corridorBounds,combineCorridor} from './environmentCorridor';
import type {RoadSnapshot} from './automaticRoadData';
const p=(longitude:number,latitude=35)=>({longitude,latitude});
it('covers a long diagonal including between vertices without fetching its whole bounding box',()=>{
 const regions=corridorBounds([[p(139,35),p(139.4,35.4)]]);
 expect(regions.length).toBeLessThan(100);expect(regions.length).toBeGreaterThan(10);
 for(let i=0;i<=1000;i++){const q=p(139+i*.0004,35+i*.0004);expect(regions.some(b=>q.longitude>=b.west&&q.longitude<=b.east&&q.latitude>=b.south&&q.latitude<=b.north)).toBe(true);}
 expect(regions.every(b=>b.east-b.west<.021&&b.north-b.south<.021)).toBe(true);
});
it('does not fill gaps between separate paths and caps work before network',()=>{
 const regions=corridorBounds([[p(139),p(139.01)],[p(140),p(140.01)]]);
 expect(regions.length).toBeLessThan(6);
 expect(()=>corridorBounds([[p(130),p(140)]])).toThrow('上限');
 expect(()=>corridorBounds([[p(NaN),p(140)]])).toThrow('座標');
});
it('keeps both providers coverage separate and scopes source geometry to the queried area',()=>{
 const b={west:139,east:139.02,south:35,north:35.02};
 const road={bounds:b,roads:[{id:'1',points:[p(138),p(141)],layer:null,level:null,bridge:null,tunnel:null}],bundle:{graphSha256:'x'}} as RoadSnapshot;
 const merged=combineCorridor([{road,water:{coverage:[],water:[]}}]);
 expect(merged.road_coverage).toEqual([b]);expect(merged.water_coverage).toEqual([]);expect(merged.roads[0].coverage).toEqual([b]);
});
