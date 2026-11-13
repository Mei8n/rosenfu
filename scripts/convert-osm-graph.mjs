#!/usr/bin/env node
/** Bounded offline snapshot conversion, not a routing or UI importer. Node >=22.18. */
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';
import { SaxesParser } from 'saxes';
import { readLocalInput, MAX_INPUT_BYTES } from './convert-osm-stations.mjs';
import { parseReferenceGraph } from '../src/referenceGraph.ts';
export const LIMITS = Object.freeze({ input: MAX_INPUT_BYTES, sidecar: 20 * 1024 * 1024, objects: 250000, refs: 500000, tags: 500000, attributes: 32, value: 4096, objectTags: 256 });
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const fail = message => { throw Error(message); };
const id = value => /^[1-9]\d{0,19}$/.test(value ?? '') ? value : fail('Invalid OSM ID');
const rail = new Set(['rail', 'subway', 'light_rail', 'tram', 'narrow_gauge']);
const road = new Set(['motorway', 'motorway_link', 'trunk', 'trunk_link', 'primary', 'primary_link', 'secondary', 'secondary_link', 'tertiary', 'tertiary_link', 'unclassified', 'residential', 'living_street', 'service']);
function coordinate(value, max) {
  if (typeof value !== 'string' || !/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)$/.test(value)) fail('Invalid coordinate');
  const n = Number(value); if (!Number.isFinite(n) || Math.abs(n) > max) fail('Coordinate outside supported range'); return n;
}
const sortId = (a, b) => a.id.length - b.id.length || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0);
function height(tags) {
  let layer = null;
  if (tags.layer !== undefined) {
    if (!/^[+-]?\d+$/.test(tags.layer) || !Number.isInteger(Number(tags.layer)) || Math.abs(Number(tags.layer)) > 100) fail('Unsupported layer');
    layer = Number(tags.layer);
  }
  const flag = (key, yes) => tags[key] === undefined ? null : tags[key] === 'no' ? false : yes.includes(tags[key]) ? true : fail(`Unsupported ${key} value`);
  const bridge = flag('bridge', ['yes', 'viaduct', 'movable', 'cantilever', 'covered', 'trestle', 'aqueduct']);
  const tunnel = flag('tunnel', ['yes', 'building_passage', 'culvert']);
  if (bridge && tunnel) fail('Conflicting bridge/tunnel');
  const level = tags.level ?? null;
  if (level !== null && (!/^[+-]?\d+(?:\.\d+)?$/.test(level) || level.length > 100)) fail('Unsupported level');
  return { layer, level, bridge, tunnel };
}
export function convertGraph(bytes, options) {
  if (bytes.byteLength > LIMITS.input) fail('Input exceeds 50 MiB');
  const xml = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  const nodes = new Map(), ways = new Map(), seen = { node: new Set(), way: new Set(), relation: new Set() };
  const counts = { objects: 0, refs: 0, tags: 0, relationsIgnored: 0, excludedWays: 0, boundarySegments: 0 };
  const stack = []; let current = null, sourceBounds = null, noteLength = 0;
  const parser = new SaxesParser({ xmlns: true });
  parser.on('error', e => { throw e; });
  parser.on('doctype', () => fail('DTD and ENTITY declarations prohibited'));
  parser.on('processinginstruction', () => fail('Processing instructions prohibited'));
  parser.on('xmldecl', d => { if (d.version !== '1.0' || (d.encoding && d.encoding.toLowerCase() !== 'utf-8')) fail('Only UTF-8 XML 1.0 supported'); });
  parser.on('text', value => {
    if (stack.at(-1) === 'note') { noteLength += value.length; if (noteLength > LIMITS.value) fail('Note length limit'); }
    else if (value.trim()) fail('Unexpected XML text');
  });
  parser.on('cdata', () => fail('CDATA prohibited'));
  parser.on('opentag', tag => {
    if (tag.prefix || tag.uri || tag.name.includes(':')) fail('Namespaces/XInclude prohibited');
    const a = Object.create(null);
    if (Object.keys(tag.attributes).length > LIMITS.attributes) fail('Attribute limit');
    for (const item of Object.values(tag.attributes)) {
      if (item.prefix || item.uri || item.name === 'xmlns' || item.name.includes(':')) fail('Namespaces prohibited');
      if (item.value.length > LIMITS.value) fail('Attribute value limit');
      a[item.name] = item.value;
    }
    const parent = stack.at(-1); stack.push(tag.name);
    if (stack.length > 3) fail('XML depth limit');
    if (!parent) { if (tag.name !== 'osm' || a.version !== '0.6') fail('Only OSM 0.6 snapshot supported'); return; }
    if (parent === 'osm') {
      if (['bounds', 'note', 'meta'].includes(tag.name)) {
        if (tag.name === 'bounds') { if (sourceBounds) fail('Duplicate bounds'); sourceBounds = { ...a }; }
        return;
      }
      if (!Object.hasOwn(seen, tag.name)) fail('Unsupported OSM element');
      if (++counts.objects > LIMITS.objects) fail('Object count limit');
      const key = id(a.id);
      if (seen[tag.name].has(key)) fail('Duplicate/history OSM ID');
      seen[tag.name].add(key);
      if (a.visible !== undefined && a.visible !== 'true') fail('Deleted object prohibited');
      if (a.action !== undefined) fail('Change/history object prohibited');
      current = { id: key, tags: Object.create(null), refs: [] };
      if (tag.name === 'node') Object.assign(current, { longitude: coordinate(a.lon, 180), latitude: coordinate(a.lat, 85.05112878) });
      if (tag.name === 'relation') counts.relationsIgnored++;
      return;
    }
    if (!current || !['node', 'way', 'relation'].includes(parent)) fail('Unexpected nested element');
    if (tag.name === 'tag') {
      if (!a.k || a.v === undefined || Object.hasOwn(current.tags, a.k)) fail('Invalid/duplicate tag');
      current.tags[a.k] = a.v;
      if (++counts.tags > LIMITS.tags || Object.keys(current.tags).length > LIMITS.objectTags) fail('Tag count limit');
    } else if (tag.name === 'nd' && parent === 'way') {
      if (++counts.refs > LIMITS.refs) fail('Reference count limit'); current.refs.push(id(a.ref));
    } else if (tag.name === 'member' && parent === 'relation') {
      if (++counts.refs > LIMITS.refs) fail('Reference count limit'); id(a.ref);
      if (!Object.hasOwn(seen, a.type)) fail('Invalid member type');
    } else fail('Unsupported nested element');
  });
  parser.on('closetag', tag => {
    if (tag.name === 'node') nodes.set(current.id, current);
    if (tag.name === 'way') ways.set(current.id, current);
    if (['node', 'way', 'relation'].includes(tag.name)) current = null;
    stack.pop();
  });
  // Small chunks avoid unbounded parser work between callbacks; input itself is bounded.
  for (let offset = 0; offset < xml.length; offset += 16384) parser.write(xml.slice(offset, offset + 16384));
  parser.close();
  const bounds = options.bounds;
  const dataset = { id: options.id, revision: options.revision, source: options.source, license: 'ODbL-1.0', bounds };
  // Reuse the real validator for options even when the selection is empty.
  parseReferenceGraph(JSON.stringify({ format: 'rosenfu-reference-graph', version: 1, dataset, nodes: [], edges: [] }));
  if (typeof options.source !== 'string' || !/^https?:\/\//.test(options.source)) fail('Source URL required');
  const inside = n => n.longitude >= bounds.west && n.longitude <= bounds.east && n.latitude >= bounds.south && n.latitude <= bounds.north;
  const used = new Set(), edges = [], originalWays = [];
  for (const way of [...ways.values()].sort(sortId)) {
    const t = way.tags;
    const modes = t.area === 'yes' || ['construction', 'proposed', 'abandoned', 'disused'].some(k => t[k] !== undefined && t[k] !== 'no') ? [] : [...(rail.has(t.railway) ? ['rail'] : []), ...(road.has(t.highway) ? ['road'] : [])];
    if (!modes.length) { counts.excludedWays++; continue; }
    if (way.refs.length < 2) fail('Selected way has fewer than two nodes');
    for (const ref of way.refs) if (!nodes.has(ref)) fail(`Missing node ${ref} on selected way ${way.id}`);
    const vertical = height(t); let contributed = false;
    for (let index = 0; index < way.refs.length - 1; index++) {
      const from = way.refs[index], to = way.refs[index + 1];
      if (!inside(nodes.get(from)) || !inside(nodes.get(to))) { counts.boundarySegments++; continue; }
      for (const mode of modes) edges.push({ id: `${way.id}/${index}/${mode}`, osmWayId: way.id, segmentIndex: index, from, to, mode, ...vertical });
      used.add(from); used.add(to); contributed = true;
      if (edges.length > 40000 || used.size > 20000) fail('Graph count limit');
    }
    if (contributed) originalWays.push({ id: way.id, nd: way.refs, tags: way.tags });
  }
  if (!edges.length) fail('No usable segments in requested bounds');
  const selectedNodes = [...used].map(key => nodes.get(key)).sort(sortId);
  const graph = { format: 'rosenfu-reference-graph', version: 1, dataset, nodes: selectedNodes.map(n => ({ osmId: n.id, longitude: n.longitude, latitude: n.latitude })), edges };
  const graphJson = JSON.stringify(graph, null, 2) + '\n'; parseReferenceGraph(graphJson);
  const sidecar = {
    format: 'rosenfu-osm-graph-source', version: 1, converter: 'osm-xml-v1', inputSha256: hash(bytes), graphSha256: hash(graphJson),
    source: options.source, sourceDate: options.sourceDate ?? null, attribution: '© OpenStreetMap contributors', license: 'ODbL-1.0', licenseUrl: 'https://opendatacommons.org/licenses/odbl/1-0/',
    requestedBounds: bounds, sourceBounds, geometryOnly: true, directionsEnforced: false, restrictionsEnforced: false, relationsPreserved: false, coverageGuaranteed: false,
    boundaryPolicy: 'original segments with both endpoints inside; outside-to-outside crossings omitted', originalInputRetention: 'Retain the original XML; relation contents are not preserved here.',
    classification: { rail: [...rail], road: [...road], areaYesExcluded: true }, counts,
    ways: originalWays, nodes: selectedNodes.map(n => ({ id: n.id, tags: n.tags })),
  };
  const sourceJson = JSON.stringify(sidecar, null, 2) + '\n';
  if (Buffer.byteLength(sourceJson) > LIMITS.sidecar) fail('Sidecar exceeds 20 MiB');
  return { graph, graphJson, sourceJson, manifest: JSON.stringify({ format: 'rosenfu-osm-graph-complete', version: 1, graphSha256: hash(graphJson), sourceSha256: hash(sourceJson) }, null, 2) + '\n' };
}
export function writeBundle(directory, result) {
  // mkdir is exclusive: pre-existing directories/symlinks/files are never reused.
  // Readers must require complete.json. A write failure leaves an explicitly incomplete bundle.
  mkdirSync(directory, { mode: 0o700 });
  for (const [name, data] of [['graph.json', result.graphJson], ['source.json', result.sourceJson], ['complete.json', result.manifest]]) writeFileSync(resolve(directory, name), data, { flag: 'wx', mode: 0o600 });
}
export function runCli(args) {
  if (args.length === 1 && args[0] === '--help') { console.log('Node >=22.18. Offline geometry-only OSM XML converter. No PBF/routing/UI import.\n--input region.osm --output NEW_DIRECTORY --id NAME --revision REV --source https://SOURCE --source-date DATE --bounds west,south,east,north\nODbL-1.0 attribution retained. Keep original XML. Both-endpoints-in-bounds filtering is NOT complete network coverage.'); return; }
  const allowed = ['input', 'output', 'id', 'revision', 'source', 'source-date', 'bounds'], o = Object.create(null);
  for (let i = 0; i < args.length; i += 2) { const key = args[i].slice(2); if (!args[i].startsWith('--') || !allowed.includes(key) || Object.hasOwn(o, key) || !args[i + 1]) fail('Invalid/duplicate/incomplete arguments'); o[key] = args[i + 1]; }
  for (const key of allowed) if (!o[key]) fail(`Missing --${key}`);
  if (!/^\d{4}-\d{2}-\d{2}(?:T[^\s]+)?$/.test(o['source-date']) || !Number.isFinite(Date.parse(o['source-date']))) fail('Invalid source date');
  const parts = o.bounds.split(','); if (parts.length !== 4 || parts.some(p => !p.trim())) fail('Four bounds values required');
  const [west, south, east, north] = parts.map(Number);
  const result = convertGraph(readLocalInput(o.input), { ...o, sourceDate: o['source-date'], bounds: { west, south, east, north } });
  writeBundle(o.output, result);
  console.log('Created geometry-only bundle; directions/restrictions and network completeness are NOT provided.');
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try { runCli(process.argv.slice(2)); } catch (error) { console.error(error.message); process.exitCode = 1; }
}
