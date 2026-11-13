import { describe, it, expect } from 'vitest';
import { mkdtempSync, readFileSync, writeFileSync, symlinkSync, rmSync, existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { convertGraph, writeBundle, LIMITS } from './convert-osm-graph.mjs';
import { createReferenceGraphRepository, parseReferenceGraph } from '../src/referenceGraph.ts';
const options = { id: 'test', revision: '1', source: 'https://example.test/extract.osm', sourceDate: '2026-11-12', bounds: { west: 0, south: 0, east: 1, north: 1 } };
const n = (id, lon = '.1', lat = '.1') => `<node id="${id}" lon="${lon}" lat="${lat}"/>`;
const w = (id, refs, tags = '<tag k="railway" v="rail"/>') => `<way id="${id}">${refs.map(ref => `<nd ref="${ref}"/>`).join('')}${tags}</way>`;
const wrap = s => `<osm version="0.6">${s}</osm>`;
const convert = s => convertGraph(Buffer.from(wrap(s)), options);
const simple = n(1) + n(2, '.2') + w(1, [1, 2]);
describe('OSM graph offline converter', () => {
  it('handles way before nodes, huge string IDs and original tags/order', () => {
    const big = '12345678901234567890';
    const r = convert(w(big, [2, 1], '<tag k="railway" v="rail"/><tag k="oneway" v="-1"/><tag k="access" v="private"/>') + n(1) + n(2, '.2'));
    expect(r.graph.edges[0]).toMatchObject({ osmWayId: big, from: '2', to: '1', segmentIndex: 0 });
    expect(JSON.parse(r.sourceJson).ways[0].tags).toMatchObject({ oneway: '-1', access: 'private' });
    expect(parseReferenceGraph(r.graphJson)).toEqual(r.graph);
    expect(convertGraph(Buffer.from(wrap(simple)), options)).toEqual(convertGraph(Buffer.from(wrap(simple)), options));
  });
  it('keeps index after boundary filtering, never interpolates or bridges missing nodes', () => {
    const r = convert(n(1, '-.1') + n(2, '.1') + n(3, '.2') + n(4, '1.1') + w(1, [1,2,3,4]));
    expect(r.graph.edges.map(e => e.segmentIndex)).toEqual([1]);
    expect(JSON.parse(r.sourceJson).counts.boundarySegments).toBe(2);
    expect(r.graph.nodes).toHaveLength(2);
    expect(() => convert(simple + w(2, [1,999]))).toThrow(/Missing node/);
    expect(() => convert(n(1, '-.1') + n(2, '1.1') + w(1, [1,2]))).toThrow(/No usable/);
  });
  it('does not connect geometric crossings, same coordinates/different ID or road/rail', async () => {
    const r = convert(n(1) + n(2, '.3') + n(3, '.2', '.05') + n(4, '.2', '.2') + n(5) + n(6, '.15') + w(1,[1,2]) + w(2,[3,4]) + w(3,[5,6]) + w(4,[1,6],'<tag k="highway" v="primary"/>') + w(5,[1,3]));
    const repo = createReferenceGraphRepository(r.graph);
    expect((await repo.connected('1/0/rail','1',100)).edges.map(e => e.id)).toEqual(['5/0/rail']);
  });
  it('preserves unknown versus zero and rejects uninterpretable height', async () => {
    const base = n(1) + n(2,'.2') + n(3,'.3');
    const r = convert(base + w(1,[1,2]) + w(2,[2,3],'<tag k="railway" v="rail"/><tag k="layer" v="0"/>'));
    expect(r.graph.edges.map(e => e.layer)).toEqual([null,0]);
    expect((await createReferenceGraphRepository(r.graph).connected('1/0/rail','2',10)).edges).toEqual([]);
    for (const t of ['<tag k="bridge" v="maybe"/>','<tag k="layer" v="abc"/>','<tag k="level" v="1;2"/>','<tag k="bridge" v="yes"/><tag k="tunnel" v="yes"/>']) expect(() => convert(base + w(1,[1,2],'<tag k="railway" v="rail"/>'+t))).toThrow();
  });
  it('rejects duplicate IDs/tags, deleted objects, tiny/invalid segments and coordinates', () => {
    for (const xml of [simple+n(1), simple.replace('<node','<node visible="false"'), simple.replace('lon=".2"','lon=".1"'), simple.replace('lon=".2"','lon="NaN"'), simple.replace('<tag k="railway" v="rail"/>','<tag k="railway" v="rail"/><tag k="railway" v="tram"/>')]) expect(() => convert(xml)).toThrow();
    expect(() => convert(simple.replace('lon=".2"','lon=".10000000001"'))).toThrow(/1 cm/);
  });
  it('rejects DTD/entity/namespace/XInclude/PI/invalid UTF8/depth/large attributes', () => {
    for (const raw of [
      '<!DOCTYPE osm [<!ENTITY x "boom">]>'+wrap(simple),
      '<!DOCTYPE osm SYSTEM "file:///etc/passwd">'+wrap(simple),
      wrap(simple.replace('rail"','&unknown;"')),
      wrap('<xi:include xmlns:xi="http://www.w3.org/2001/XInclude" href="file:///etc/passwd"/>'),
      '<?fetch test?>'+wrap(simple), '<osmChange version="0.6"/>',
      wrap('<node id="1" lon=".1" lat=".1"><tag k="a" v="b"><tag k="c" v="d"/></tag></node>'),
      wrap(simple.replace('v="rail"',`v="${'x'.repeat(LIMITS.value+1)}"`)),
    ]) expect(() => convertGraph(Buffer.from(raw),options)).toThrow();
    expect(() => convertGraph(Buffer.from([0xff]),options)).toThrow();
    expect(() => convertGraph(Buffer.alloc(LIMITS.input+1),options)).toThrow(/50 MiB/);
  });
  it('excludes unsupported ways and reports relation/direction/coverage limitations', () => {
    const r = convert(simple + w(2,[1,2],'<tag k="highway" v="footway"/>') + '<relation id="1"><member type="way" ref="1" role="from"/><tag k="type" v="restriction"/></relation>');
    expect(JSON.parse(r.sourceJson)).toMatchObject({ geometryOnly: true, directionsEnforced: false, restrictionsEnforced: false, relationsPreserved: false, coverageGuaranteed: false, counts: { excludedWays: 1, relationsIgnored: 1 } });
  });
  it('enforces graph counts and serialized sidecar size without truncation', () => {
    const base = n(1) + n(2,'.2');
    expect(() => convert(base + Array.from({length:40001},(_,i)=>w(i+1,[1,2])).join(''))).toThrow(/Graph count limit/);
    const bigA='12345678901234567890', bigB='12345678901234567891';
    expect(() => convert(n(bigA)+n(bigB,'.2')+Array.from({length:19000},(_,i)=>w(String(12345678901234560000n+BigInt(i)),[bigA,bigB])).join(''))).toThrow(/5 MiB/);
    const tags = '<tag k="railway" v="rail"/><tag k="description" v="'+'x'.repeat(4000)+'"/>';
    expect(() => convert(base + Array.from({length:5300},(_,i)=>w(i+1,[1,2],tags)).join(''))).toThrow(/Sidecar exceeds/);
    expect(() => convert(base + w(1,[1,2], Array.from({length:257},(_,i)=>`<tag k="key${i}" v="x"/>`).join('')))).toThrow(/Tag count limit/);
  }, 30000);
  it('executes the actual Node CLI and validates the resulting graph', () => {
    const dir=mkdtempSync(join(tmpdir(),'graph-cli-'));
    try {
      const input=join(dir,'input.osm'), output=join(dir,'output'); writeFileSync(input,wrap(simple));
      const args=['scripts/convert-osm-graph.mjs','--input',input,'--output',output,'--id','test','--revision','1','--source',options.source,'--source-date','2026-11-12','--bounds','0,0,1,1'];
      const first=spawnSync(process.execPath,args,{encoding:'utf8'});
      expect(first.status,first.stderr).toBe(0); expect(parseReferenceGraph(readFileSync(join(output,'graph.json'),'utf8')).edges).toHaveLength(1);
      const second=spawnSync(process.execPath,args,{encoding:'utf8'}); expect(second.status).toBe(1);
    } finally { rmSync(dir,{recursive:true,force:true}); }
  });
  it('creates only a fresh bundle and refuses an existing destination without changes', () => {
    const dir = mkdtempSync(join(tmpdir(),'graph-converter-'));
    try {
      const out = join(dir,'bundle'); const r=convert(simple); writeBundle(out,r);
      expect(readFileSync(join(out,'complete.json'),'utf8')).toBe(r.manifest);
      expect(() => writeBundle(out,r)).toThrow();
      expect(readFileSync(join(out,'graph.json'),'utf8')).toBe(r.graphJson);
      const alias=join(dir,'alias'); symlinkSync(out,alias,'dir'); expect(() => writeBundle(alias,r)).toThrow();
      expect(readFileSync(join(out,'graph.json'),'utf8')).toBe(r.graphJson);
      const occupied=join(dir,'occupied'); writeFileSync(occupied,'original');
      expect(() => writeBundle(occupied,r)).toThrow(); expect(readFileSync(occupied,'utf8')).toBe('original');
      // Invalid write payload simulates a write failure before completion marker.
      const partial=join(dir,'partial'); expect(() => writeBundle(partial,{...r,sourceJson:undefined})).toThrow();
      expect(existsSync(join(partial,'complete.json'))).toBe(false);
    } finally { rmSync(dir,{recursive:true,force:true}); }
  });
});
