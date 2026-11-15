import {it,expect} from 'vitest';
import {readFileSync} from 'node:fs';
import {gunzipSync} from 'node:zlib';
import {decodeRailGraph,createRailNetwork} from '../src/realRailGraph';
// Opt-in live evidence: the release file stays outside git and is never needed by CI.
it.skipIf(!process.env.ROSENFU_REAL_RAIL_FIXTURE)('decodes the verified published Japan graph and routes Tokyo station tracks',()=>{
 const data=gunzipSync(readFileSync(process.env.ROSENFU_REAL_RAIL_FIXTURE!));const graph=decodeRailGraph(data.buffer.slice(data.byteOffset,data.byteOffset+data.byteLength));expect(graph.chains.length).toBeGreaterThan(100000);
 const net=createRailNetwork(graph),view=net.visible({west:139.76,east:139.775,south:35.675,north:35.685});expect(view.truncated).toBe(false);expect(view.chains.length).toBeGreaterThan(10);
 const chain=view.chains.find(c=>c.length>100)!;const a={chain:chain.id,position:0,point:chain.points[0]},b={chain:chain.id,position:1,point:chain.points.at(-1)!};expect(net.route(a,b).points).toEqual(chain.points);
 const next=graph.chains.find(c=>c.id!==chain.id&&c.from===chain.to&&c.length>25);expect(next).toBeDefined();const route=net.route(a,{chain:next!.id,position:1,point:next!.points.at(-1)!});expect(route.spans.length).toBeGreaterThan(1);expect(route.points.at(-1)).toEqual(next!.points.at(-1));
});
