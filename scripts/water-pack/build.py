#!/usr/bin/env python3
"""Build a bounded water evidence pack. Never certifies geographic completeness."""
import argparse
import hashlib
import json
import math
from pathlib import Path
from urllib.parse import urlsplit

MAX_BYTES = 5 * 1024 * 1024
MAX_FEATURES = 10000
MAX_COORDS = 100000
WATER_SELECTION = 'natural=water | waterway=riverbank | landuse=reservoir,basin'


def encoded(value):
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'), allow_nan=False) + '\n').encode()


def digest_file(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()


def water_tags(tags):
    return tags.get('natural') == 'water' or tags.get('waterway') == 'riverbank' or tags.get('landuse') in ('reservoir', 'basin')


def check_bounds(bounds):
    if len(bounds) != 4 or not all(type(v) in (int, float) and math.isfinite(v) for v in bounds):
        raise ValueError('Invalid selection bounds')
    w, s, e, n = bounds
    if not (-180 <= w < e <= 180 and -85 <= s < n <= 85 and e-w <= .2 and n-s <= .2):
        raise ValueError('Selection must be within 0.2 degrees, without antimeridian crossing')


def geometry_points(geometry):
    kind, coordinates = geometry.get('type'), geometry.get('coordinates')
    if kind == 'LineString':
        lines = [coordinates]
    elif kind == 'Polygon':
        if not isinstance(coordinates, list) or not coordinates:
            raise ValueError('Empty polygon')
        lines = coordinates
    elif kind == 'MultiPolygon':
        if not isinstance(coordinates, list) or not coordinates or any(not isinstance(p, list) or not p for p in coordinates):
            raise ValueError('Empty multipolygon')
        lines = [r for p in coordinates for r in p]
    else:
        raise ValueError('Unsupported geometry')
    points = []
    for line in lines:
        if not isinstance(line, list) or len(line) < (2 if kind == 'LineString' else 4):
            raise ValueError('Short geometry')
        if kind != 'LineString' and line[0] != line[-1]:
            raise ValueError('Unclosed ring')
        for point in line:
            if not isinstance(point, (list, tuple)) or len(point) != 2 or not all(type(v) in (int, float) and math.isfinite(v) for v in point):
                raise ValueError('Invalid coordinate')
            if not (-180 <= point[0] <= 180 and -85 <= point[1] <= 85):
                raise ValueError('Coordinate outside supported range')
            points.append(point)
            if len(points) > MAX_COORDS:
                raise ValueError('Coordinate limit')
    if len(points) > MAX_COORDS:
        raise ValueError('Coordinate limit')
    return points


class Collector:
    def __init__(self, bounds):
        check_bounds(bounds)
        self.bounds, self.items, self.ids, self.count = list(bounds), [], set(), 0
        self.polygons = 0
        self.issues = []
        self.source_header = {}

    def add(self, identity, kind, geometry, tags):
        if not isinstance(identity, str) or not identity or len(identity) > 256:
            raise ValueError('Invalid identity')
        if kind not in ('water-area', 'waterway-line', 'coastline'):
            raise ValueError('Invalid evidence kind')
        if (kind == 'water-area') != (geometry.get('type') in ('Polygon', 'MultiPolygon')):
            raise ValueError('Kind and geometry mismatch')
        points = geometry_points(geometry)
        w, s, e, n = self.bounds
        if max(p[0] for p in points) < w or min(p[0] for p in points) > e or max(p[1] for p in points) < s or min(p[1] for p in points) > n:
            return
        # Bounding-box selection intentionally over-includes; full source geometry is retained.
        if (kind, identity) in self.ids:
            raise ValueError('Duplicate evidence ID')
        if not isinstance(tags, dict) or len(tags) > 128 or any(not isinstance(k, str) or not k or not isinstance(v, str) or len(k) > 256 or len(v) > 4096 for k, v in tags.items()):
            raise ValueError('Invalid source tags')
        if kind == 'water-area':
            self.polygons += 1 if geometry['type'] == 'Polygon' else len(geometry['coordinates'])
            if self.polygons > MAX_FEATURES:
                raise ValueError('Expanded polygon limit exceeded')
        self.count += len(points)
        if self.count > MAX_COORDS or len(self.items) >= MAX_FEATURES:
            raise ValueError('Evidence limit exceeded; choose a smaller region')
        self.ids.add((kind, identity))
        self.items.append(dict(id=identity, kind=kind, geometry=geometry, tags=tags))


def read_geojson(path, collector):
    if path.stat().st_size > MAX_BYTES:
        raise ValueError('GeoJSON input exceeds 5 MiB')
    value = json.loads(path.read_text(encoding='utf-8'))
    if 'crs' in value:
        raise ValueError('Only RFC 7946 longitude/latitude GeoJSON without crs is supported')
    if value.get('type') != 'FeatureCollection' or not isinstance(value.get('features'), list) or len(value['features']) > MAX_FEATURES:
        raise ValueError('Expected bounded FeatureCollection')
    for feature in value['features']:
        if feature.get('type') != 'Feature':
            raise ValueError('Expected Feature')
        props = feature.get('properties', {})
        collector.add(feature.get('id'), props.get('evidence_kind'), feature.get('geometry', {}), props.get('source_tags', {}))


def read_osm(path, collector):
    try:
        import osmium
    except ImportError as error:
        raise ValueError('PBF input requires separately installed osmium; see requirements.txt') from error
    with osmium.io.Reader(str(path)) as reader:
        header = reader.header()
        collector.source_header = {key: header.get(key) for key in ('osmosis_replication_timestamp', 'generator') if header.get(key)}
    factory = osmium.geom.GeoJSONFactory()

    class Handler(osmium.SimpleHandler):
        def way(self, way):
            tags = dict(way.tags)
            kind = 'coastline' if tags.get('natural') == 'coastline' else 'waterway-line' if 'waterway' in tags and tags['waterway'] != 'riverbank' else None
            # A non-closed water-tagged way cannot provide a surface polygon.
            if not kind and water_tags(tags) and not way.is_closed():
                kind = 'waterway-line'
            if not kind:
                return
            try:
                geometry = json.loads(factory.create_linestring(way))
            except (osmium.InvalidLocationError, RuntimeError):
                if len(collector.issues) >= 1000:
                    raise ValueError('Too many unresolved source geometries')
                collector.issues.append(dict(id=f'way/{way.id}', reason='missing-location-or-invalid-line'))
                return
            collector.add(f'way/{way.id}', kind, geometry, tags)

        def area(self, area):
            tags = dict(area.tags)
            if not water_tags(tags):
                return
            identity = f'{"way" if area.from_way() else "relation"}/{area.orig_id()}'
            try:
                geometry = json.loads(factory.create_multipolygon(area))
            except (osmium.InvalidLocationError, RuntimeError):
                if len(collector.issues) >= 1000:
                    raise ValueError('Too many unresolved source geometries')
                collector.issues.append(dict(id=identity, reason='invalid-area'))
                return
            collector.add(identity, 'water-area', geometry, tags)

    # Pyosmium assembles multipolygon relations and retains inner rings. Failed assembly
    # can omit areas without a callback; hence this is observations, never completeness.
    Handler().apply_file(str(path), locations=True, idx='flex_mem')


def build(path, bounds, revision, source_url, license_name, license_url, attribution, output, max_input_bytes=768*1024*1024):
    path, output = Path(path), Path(output)
    if output.exists():
        raise ValueError('Output already exists')
    if not path.is_file() or path.stat().st_size > max_input_bytes:
        raise ValueError('Input missing or exceeds size limit')
    for name, value in [('revision', revision), ('source_url', source_url), ('license', license_name), ('license_url', license_url), ('attribution', attribution)]:
        if not isinstance(value, str) or not value or len(value) > 4096:
            raise ValueError(f'Invalid {name}')
    for url in (source_url, license_url):
        parsed = urlsplit(url)
        if parsed.scheme != 'https' or not parsed.hostname or parsed.username or parsed.password:
            raise ValueError('Source and license URLs must be HTTPS without credentials')
    original_hash = digest_file(path)
    collector = Collector(bounds)
    if path.suffix == '.pbf':
        if license_name != 'ODbL-1.0':
            raise ValueError('OSM PBF requires ODbL-1.0 provenance')
        read_osm(path, collector)
        selection = WATER_SELECTION + '; waterway=* lines; natural=coastline lines'
    else:
        read_geojson(path, collector)
        selection = 'Explicit water-area/waterway-line/coastline GeoJSON observations'
    if digest_file(path) != original_hash:
        raise ValueError('Input changed during extraction')
    source = dict(url=source_url, file_sha256=original_hash, header=collector.source_header, bytes=path.stat().st_size, license=license_name, license_url=license_url, attribution=attribution, selection=selection)
    manifest_bytes, payload, manifest = pack_bytes(collector, revision, source)
    # Existing paths are never overwritten; manifest is written last as the commit marker.
    output.mkdir(parents=True, exist_ok=False)
    (output/'evidence.json').write_bytes(payload)
    (output/'manifest.json').write_bytes(manifest_bytes)
    return dict(manifest_sha256=hashlib.sha256(manifest_bytes).hexdigest(), payload_sha256=manifest['payload']['sha256'], counts=manifest['payload']['counts'], source_issues=len(collector.issues), geographic_completeness='unknown')


def pack_bytes(collector, revision, source):
    collector.items.sort(key=lambda item: (item['id'], item['kind']))
    data = dict(format='rosenfu-water-evidence', version=1, revision=revision, bounds=list(collector.bounds), geographic_completeness='unknown', coverage=[], observations=collector.items, source_issues=collector.issues)
    payload = encoded(data)
    if len(payload) > MAX_BYTES:
        raise ValueError('Pack payload exceeds 5 MiB')
    counts = {kind: sum(o['kind'] == kind for o in collector.items) for kind in ('water-area', 'waterway-line', 'coastline')}
    manifest = dict(format='rosenfu-water-evidence-manifest', version=1, revision=revision, bounds=list(collector.bounds), geographic_completeness='unknown', coverage=[], source=source, payload=dict(file='evidence.json', sha256=hashlib.sha256(payload).hexdigest(), bytes=len(payload), counts=counts), limitations=['No absence-of-water or dry-land certification', 'Full geometry preserved; bounds select by bounding-box overlap only', 'Waterway widths and coastline water-side polygons are unresolved', 'Source completeness, date and classification are not independently guaranteed'])
    manifest_bytes = encoded(manifest)
    return manifest_bytes, payload, manifest


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('input', type=Path)
    parser.add_argument('--bounds', nargs=4, type=float, required=True, metavar=('WEST','SOUTH','EAST','NORTH'))
    parser.add_argument('--revision', required=True)
    parser.add_argument('--source-url', required=True)
    parser.add_argument('--license', required=True)
    parser.add_argument('--license-url', required=True)
    parser.add_argument('--attribution', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--max-input-bytes', type=int, default=768*1024*1024)
    args = parser.parse_args()
    try:
        print(json.dumps(build(args.input,args.bounds,args.revision,args.source_url,args.license,args.license_url,args.attribution,args.output,args.max_input_bytes), ensure_ascii=False))
    except (ValueError, OSError, KeyError, TypeError) as error:
        parser.exit(1, f'water pack: {error}\n')
