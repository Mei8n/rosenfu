import {describe,it,expect} from 'vitest';
import {createRailNetwork,decodeRailGraph,type RailGraph} from './realRailGraph';
import {distanceM} from './alignment';
function graph():RailGraph{
 const chains=[{from:0,to:1,xy:[[139,35],[139.001,35.0001],[139.002,35]]},{from:1,to:2,xy:[[139.002,35],[139.003,35],[139.004,35]]},{from:3,to:4,xy:[[139.001,34.999],[139.001,35.001]]},{from:5,to:6,xy:[[139.004,35],[139.005,35]]}].map((c,id)=>{const points=c.xy.map(([longitude,latitude])=>({longitude,latitude}));let length=0;const positions=points.map((p,i)=>{if(i)length+=distanceM(points[i-1],p);return length;});return {id,from:c.from,to:c.to,points,positions:positions.map(n=>n/length),length,bounds:{west:Math.min(...points.map(p=>p.longitude)),east:Math.max(...points.map(p=>p.longitude)),south:Math.min(...points.map(p=>p.latitude)),north:Math.max(...points.map(p=>p.latitude))}};});return {chains,junctionCount:7};
}
describe('independent rail network geometry',()=>{
 it('crosses chains only through explicit junction identity, preserving bends and direction',()=>{const net=createRailNetwork(graph()),a={chain:0,position:0,point:{longitude:139,latitude:35}},b={chain:1,position:1,point:{longitude:139.004,latitude:35}};const route=net.route(a,b);expect(route.points).toEqual([...graph().chains[0].points,...graph().chains[1].points.slice(1)]);expect(route.spans.map(s=>s.chain_id)).toEqual([0,1]);expect(net.route(b,a).points).toEqual([...route.points].reverse());});
 it('does not join a crossing or identical coordinates with different junction IDs',()=>{const net=createRailNetwork(graph()),start={chain:0,position:0,point:{longitude:139,latitude:35}};for(const chain of [2,3])expect(()=>net.route(start,{...start,chain,position:1})).toThrow('接続する経路がありません');});
 it('snaps only to currently drawn chain IDs and respects radius',()=>{const net=createRailNetwork(graph()),p={longitude:139.003,latitude:35};expect(net.nearest(p,10,[0])).toBeNull();expect(net.nearest(p,10,[1])?.chain).toBe(1);expect(net.nearest({...p,latitude:35.01},10,[1])).toBeNull();});
 it('supports same chain slices without detouring, rejects zero length and invalid picks',()=>{const net=createRailNetwork(graph()),a={chain:0,position:.1,point:{longitude:139,latitude:35}},b={...a,position:.9};const route=net.route(a,b);expect(route.spans).toHaveLength(1);expect(route.points[1]).toEqual(graph().chains[0].points[1]);expect(()=>net.route(a,a)).toThrow('20m');expect(()=>net.route({...a,position:NaN},b)).toThrow('不正');});
 it('rejects truncated, unsupported and unbounded binary files before allocation',()=>{expect(()=>decodeRailGraph(new ArrayBuffer(2))).toThrow();const data=new ArrayBuffer(24),v=new DataView(data);v.setUint32(0,0x47525754,true);v.setUint32(4,1,true);v.setUint32(8,0xffffffff,true);expect(()=>decodeRailGraph(data)).toThrow('上限');});
});

it('retains the chosen arrival end for a loop chain',()=>{
 const g=graph(),loop=g.chains[1];loop.to=loop.from;loop.points=[{longitude:139.002,latitude:35},{longitude:139.003,latitude:35.001},{longitude:139.002,latitude:35}];loop.positions=[0,.5,1];loop.length=300;
 const result=createRailNetwork(g).route({chain:0,position:0,point:g.chains[0].points[0]},{chain:1,position:.9,point:loop.points[0]});
 expect(result.spans.at(-1)?.from_fraction).toBe(1);expect(result.spans.at(-1)?.to_fraction).toBe(.9);
});
it('decodes a minimal documented TWRG chain and rejects malformed offsets and trailing bytes',()=>{
 const bytes:number[]=[];const u32=(n:number)=>{for(let i=0;i<4;i++)bytes.push((n>>>(8*i))&255);};const zigzag=(n:number)=>{let v=n<0?-n*2-1:n*2;while(v>=128){bytes.push((v%128)|128);v=Math.floor(v/128);}bytes.push(v);};
 [0x47525754,1,2,1,2,0,139000000,139001000,35000000,35000000,0,1].forEach(u32);bytes.push(0,0,0);[0,2].forEach(u32);[139000000,35000000,1000,0].forEach(zigzag);
 const raw=Uint8Array.from(bytes);expect(decodeRailGraph(raw.buffer).chains[0].points).toEqual([{longitude:139,latitude:35},{longitude:139.001,latitude:35}]);
 expect(()=>decodeRailGraph(Uint8Array.from([...bytes,0]).buffer)).toThrow('末尾');
 const bad=raw.slice();new DataView(bad.buffer).setUint32(55,3,true);expect(()=>decodeRailGraph(bad.buffer)).toThrow();
});
