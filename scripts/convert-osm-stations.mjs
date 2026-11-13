#!/usr/bin/env node
/** Local-only converter. Does not fetch OSM data or claim to validate its licence. */
import { openSync, closeSync, fstatSync, readSync, writeFileSync, constants } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

export const MAX_INPUT_BYTES = 50 * 1024 * 1024;
const MAX_OUTPUT_BYTES = 5 * 1024 * 1024;
const MAX_STATIONS = 20000;
const MAX_LAT = 85.05112878;
function text(value, name, max = 200) {
  if (typeof value !== 'string' || !value.trim() || value.length > max) throw Error(`${name}: nonempty string of at most ${max} characters required`);
  return value;
}
function validObject(value) { return value !== null && typeof value === 'object' && !Array.isArray(value); }
function validNumber(value, min, max) { return typeof value === 'number' && Number.isFinite(value) && value >= min && value <= max; }

export function convertStations(input, metadata) {
  if (!validObject(input) || input.type !== 'FeatureCollection' || !Array.isArray(input.features)) throw Error('Input must be a GeoJSON FeatureCollection');
  const { west, south, east, north } = metadata.bounds ?? {};
  if (!validNumber(west, -180, 180) || !validNumber(east, -180, 180) || !validNumber(south, -MAX_LAT, MAX_LAT) || !validNumber(north, -MAX_LAT, MAX_LAT) || west >= east || south >= north) throw Error('Invalid bounds; antimeridian-crossing bounds are not supported');
  const dataset = {
    id: text(metadata.id, 'id'), revision: text(metadata.revision, 'revision'),
    source: text(metadata.source, 'source', 1000), license: text(metadata.license, 'license', 1000),
    bounds: { west, south, east, north },
  };
  const stations = [], seen = new Set(), errors = [], warnings = [];
  let errorCount = 0, warningCount = 0, skippedNonStation = 0, skippedOutside = 0;
  const error = message => { errorCount++; if (errors.length < 50) errors.push(message); };
  const warn = message => { warningCount++; if (warnings.length < 50) warnings.push(message); };
  for (const [index, feature] of input.features.entries()) {
    const label = `feature ${index + 1}`;
    if (!validObject(feature) || feature.type !== 'Feature' || !validObject(feature.properties)) { error(`${label}: invalid Feature/properties`); continue; }
    const properties = feature.properties;
    const tags = validObject(properties.tags) ? properties.tags : properties;
    if (tags.railway !== 'station' && tags.railway !== 'halt') { skippedNonStation++; continue; }
    const geometry = feature.geometry;
    if (!validObject(geometry) || geometry.type !== 'Point') { warn(`${label}: station is not Point; skipped without inventing a representative point`); continue; }
    const coordinates = geometry.coordinates;
    if (!Array.isArray(coordinates) || coordinates.length < 2 || !validNumber(coordinates[0], -180, 180) || !validNumber(coordinates[1], -MAX_LAT, MAX_LAT)) { error(`${label}: invalid coordinates`); continue; }
    const [longitude, latitude] = coordinates;
    if (longitude < west || longitude > east || latitude < south || latitude > north) { skippedOutside++; continue; }
    // Full string IDs avoid loss of precision and node/way/relation collisions.
    const ids = [feature.id, properties['@id']].filter(value => value !== undefined);
    const id = ids[0];
    if (typeof id !== 'string' || !/^(node|way|relation)\/[1-9]\d{0,19}$/.test(id) || ids.some(value => value !== id)) { error(`${label}: unambiguous string OSM ID (e.g. node/123) required`); continue; }
    if (seen.has(id)) { error(`${label}: duplicate OSM ID ${id}`); continue; }
    seen.add(id);
    const name = tags['name:ja'] ?? tags.name;
    if (typeof name !== 'string' || !name.trim() || name.length > 200) { error(`${label}: nonempty name/name:ja of at most 200 characters required`); continue; }
    const [osmType, osmId] = id.split('/');
    stations.push({ osmType, osmId, name, longitude, latitude, kind: tags.railway });
  }
  if (stations.length > MAX_STATIONS) error(`Output exceeds ${MAX_STATIONS} stations; choose a smaller bounds`);
  if (stations.length === 0) error('No usable station Points in bounds');
  if (errorCount) throw Error(`Conversion rejected (${errorCount} errors; first ${errors.length} shown):\n${errors.join('\n')}`);
  stations.sort((a, b) => `${a.osmType}/${a.osmId}`.localeCompare(`${b.osmType}/${b.osmId}`, 'en'));
  const pack = { format: 'rosenfu-reference-stations', version: 1, dataset, stations };
  const json = JSON.stringify(pack, null, 2) + '\n';
  if (Buffer.byteLength(json, 'utf8') > MAX_OUTPUT_BYTES) throw Error('Output exceeds 5 MiB; choose a smaller bounds');
  return { pack, json, report: { inputFeatures: input.features.length, outputStations: stations.length, skippedNonStation, skippedOutside, warningCount, warnings } };
}

const HELP = `Local OSM station Point converter (no network access)
node scripts/convert-osm-stations.mjs --input input.geojson --output stations.json \\
  --id osaka --revision 2026-11-12 --bounds 135.3,34.5,135.7,34.9 \\
  --source "OpenStreetMap; source URL and extraction details" --license ODbL-1.0
Bounds order: west,south,east,north. Input must be lawfully obtained.
Output must not exist. station/halt Points only; non-Point stations are reported and skipped.
`;
export function readLocalInput(path) {
  // Nonblocking open prevents waiting indefinitely on a FIFO before its type is checked.
  const fd = openSync(path, constants.O_RDONLY | (constants.O_NONBLOCK ?? 0));
  try {
    const stat = fstatSync(fd);
    if (!stat.isFile()) throw Error('Input must be a regular file');
    if (stat.size > MAX_INPUT_BYTES) throw Error('Input exceeds 50 MiB');
    // Read at most limit + 1 even if the file grows after fstat; use the same descriptor.
    const chunks = [];
    let total = 0;
    while (total <= MAX_INPUT_BYTES) {
      const chunk = Buffer.alloc(Math.min(64 * 1024, MAX_INPUT_BYTES + 1 - total));
      const length = readSync(fd, chunk, 0, chunk.length, null);
      if (length === 0) break;
      total += length;
      chunks.push(chunk.subarray(0, length));
    }
    if (total > MAX_INPUT_BYTES) throw Error('Input exceeds 50 MiB');
    return Buffer.concat(chunks, total);
  } finally { closeSync(fd); }
}

export function runCli(args) {
  if (args.length === 1 && args[0] === '--help') { process.stdout.write(HELP); return; }
  const options = {};
  const allowed = new Set(['input', 'output', 'id', 'revision', 'bounds', 'source', 'license']);
  for (let index = 0; index < args.length; index += 2) {
    const key = args[index]?.replace(/^--/, '');
    if (!args[index]?.startsWith('--') || !allowed.has(key) || Object.hasOwn(options, key) || args[index + 1] === undefined) throw Error('Invalid, duplicate or incomplete arguments; use --help');
    options[key] = args[index + 1];
  }
  for (const key of allowed) if (!options[key]) throw Error(`Missing --${key}; use --help`);
  if (resolve(options.input) === resolve(options.output)) throw Error('Input and output must differ');
  const bytes = readLocalInput(options.input);
  const parts = options.bounds.split(',');
  if (parts.length !== 4 || parts.some(part => !part.trim())) throw Error('Four bounds values required');
  const [west, south, east, north] = parts.map(Number);
  const result = convertStations(JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)), { ...options, bounds: { west, south, east, north } });
  writeFileSync(options.output, result.json, { encoding: 'utf8', flag: 'wx' });
  process.stderr.write(JSON.stringify({ input: options.input, output: options.output, ...result.report }, null, 2) + '\n');
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try { runCli(process.argv.slice(2)); } catch (error) { process.stderr.write(`${error.message}\n`); process.exitCode = 1; }
}
