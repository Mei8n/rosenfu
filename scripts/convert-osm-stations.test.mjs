import { describe, it, expect } from 'vitest';
import { mkdtempSync, writeFileSync, readFileSync, rmSync, existsSync, truncateSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { convertStations, readLocalInput, MAX_INPUT_BYTES } from './convert-osm-stations.mjs';
import { parseReferencePack } from '../src/referenceStations';

// Synthetic fixtures, not real-world station data.
const metadata = { id: 'test-only', revision: 'test', source: 'Synthetic test fixture, not OSM data', license: 'test-only', bounds: { west: 130, south: 30, east: 140, north: 40 } };
const point = (id = 'node/9007199254740993', overrides = {}) => ({ type: 'Feature', id, properties: { railway: 'station', name: 'Test fixture' }, geometry: { type: 'Point', coordinates: [135, 35] }, ...overrides });
const collection = (...features) => ({ type: 'FeatureCollection', features });

describe('local station converter', () => {
  it('preserves string IDs and produces a pack accepted by the UI parser', () => {
    const result = convertStations(collection(point()), metadata);
    expect(parseReferencePack(result.json)).toEqual(result.pack);
    expect(result.pack.stations[0].osmId).toBe('9007199254740993');
    expect(result.pack.dataset.source).toBe(metadata.source);
  });
  it('supports nested tags and @id, prefers Japanese name and keeps HTML as text', () => {
    const feature = point(undefined, { properties: { '@id': 'relation/2', tags: { railway: 'halt', name: 'name', 'name:ja': '<b>test</b>' } } });
    delete feature.id;
    const result = convertStations(collection(feature), metadata);
    expect(result.pack.stations[0]).toMatchObject({ osmType: 'relation', osmId: '2', kind: 'halt', name: '<b>test</b>' });
  });
  it('reports omitted non-Point stations and filters other features/outside bounds', () => {
    const result = convertStations(collection(point(), point('way/2', { geometry: { type: 'Polygon', coordinates: [] } }), point('node/3', { properties: { railway: 'platform' } }), point('node/4', { geometry: { type: 'Point', coordinates: [145, 35] } })), metadata);
    expect(result.report).toMatchObject({ outputStations: 1, warningCount: 1, skippedNonStation: 1, skippedOutside: 1 });
  });
  it('rejects duplicates without merging stations by name', () => {
    expect(() => convertStations(collection(point(), point()), metadata)).toThrow('duplicate OSM ID');
    expect(convertStations(collection(point('node/1'), point('node/2')), metadata).pack.stations).toHaveLength(2);
  });
  it.each([123, 'node/0', 'node/01', 'node/123456789012345678901', '123'])('rejects unsafe or malformed ID %s', id => {
    expect(() => convertStations(collection(point(id)), metadata)).toThrow('string OSM ID');
  });
  it('rejects disagreeing ID sources, missing name, bad coordinates and bad bounds', () => {
    expect(() => convertStations(collection(point('node/1', { properties: { '@id': 'node/2', railway: 'station', name: 'test' } })), metadata)).toThrow('unambiguous');
    expect(() => convertStations(collection(point('node/1', { properties: { railway: 'station' } })), metadata)).toThrow('nonempty name');
    expect(() => convertStations(collection(point('node/1', { geometry: { type: 'Point', coordinates: [Infinity, 35] } })), metadata)).toThrow('coordinates');
    expect(() => convertStations(collection(point()), { ...metadata, bounds: { west: 140, east: 130, south: 30, north: 40 } })).toThrow('bounds');
  });
  it('rejects no usable points and too many stations', () => {
    expect(() => convertStations(collection(), metadata)).toThrow('No usable');
    expect(() => convertStations(collection(...Array.from({ length: 20001 }, (_, i) => point(`node/${i + 1}`))), metadata)).toThrow('20000');
  });
  it('rejects output larger than the UI byte limit', () => {
    const features = Array.from({ length: 20000 }, (_, i) => point(`node/${i + 1}`, { properties: { railway: 'station', name: '駅'.repeat(200) } }));
    expect(() => convertStations(collection(...features), metadata)).toThrow('5 MiB');
  });
  it('rejects nonregular and oversized inputs before reading', () => {
    const dir = mkdtempSync(join(tmpdir(), 'rosenfu-input-'));
    try {
      expect(() => readLocalInput(dir)).toThrow('regular file');
      const file = join(dir, 'large.geojson');
      writeFileSync(file, '');
      truncateSync(file, MAX_INPUT_BYTES + 1);
      expect(() => readLocalInput(file)).toThrow('50 MiB');
      if (process.platform !== 'win32') expect(() => readLocalInput('/dev/null')).toThrow('regular file');
    } finally { rmSync(dir, { recursive: true, force: true }); }
  });
  it('CLI writes only new output and leaves existing files untouched on failure', () => {
    const dir = mkdtempSync(join(tmpdir(), 'rosenfu-converter-'));
    try {
      const input = join(dir, 'input.geojson'), output = join(dir, 'output.json');
      writeFileSync(input, JSON.stringify(collection(point())));
      const args = [resolve('scripts/convert-osm-stations.mjs'), '--input', input, '--output', output, '--id', 'test-only', '--revision', 'test', '--source', metadata.source, '--license', metadata.license, '--bounds', '130,30,140,40'];
      const first = spawnSync(process.execPath, args, { encoding: 'utf8' });
      expect(first.status).toBe(0);
      expect(parseReferencePack(readFileSync(output, 'utf8')).stations).toHaveLength(1);
      writeFileSync(output, 'keep me');
      expect(spawnSync(process.execPath, args).status).toBe(1);
      expect(readFileSync(output, 'utf8')).toBe('keep me');
      rmSync(output);
      writeFileSync(input, JSON.stringify(collection(point(), point())));
      expect(spawnSync(process.execPath, args).status).toBe(1);
      expect(existsSync(output)).toBe(false);
    } finally { rmSync(dir, { recursive: true, force: true }); }
  });
});
