#!/usr/bin/env python3
"""Extract explicit small cells in one source pass; never certify coverage."""
import argparse
import hashlib
import json
from pathlib import Path
from urllib.parse import urlsplit
from build import Collector, MAX_BYTES, WATER_SELECTION, digest_file, encoded, geometry_points, pack_bytes, read_geojson, read_osm


class Grid:
    def __init__(self, cells):
        if not isinstance(cells, list) or not 1 <= len(cells) <= 256:
            raise ValueError('Supply 1..256 explicit cell bounds per batch')
        self.cells = [Collector(bounds) for bounds in cells]
        if len({tuple(c.bounds) for c in self.cells}) != len(cells):
            raise ValueError('Duplicate cell')
        self.issues, self.source_header = [], {}
        self.copies = self.coordinates = 0

    def add(self, identity, kind, geometry, tags):
        # Validate once, then route by bbox. No clipping or absence inference.
        points = geometry_points(geometry)
        west, east = min(p[0] for p in points), max(p[0] for p in points)
        south, north = min(p[1] for p in points), max(p[1] for p in points)
        for cell in self.cells:
            w, s, e, n = cell.bounds
            if east < w or west > e or north < s or south > n:
                continue
            self.copies += 1
            self.coordinates += len(points)
            if self.copies > 100000 or self.coordinates > 1000000:
                raise ValueError('Batch output limit; use fewer cells')
            cell.add(identity, kind, geometry, tags)


def build_grid(path, cells, revision, source_url, license_name, license_url, attribution, output):
    path, output = Path(path), Path(output)
    if output.exists() or not path.is_file() or not 0 < path.stat().st_size <= 768*1024*1024:
        raise ValueError('New output and bounded regional source required')
    for value in (revision, source_url, license_name, license_url, attribution):
        if not isinstance(value, str) or not value or len(value) > 4096:
            raise ValueError('Invalid provenance')
    for url in (source_url, license_url):
        parsed = urlsplit(url)
        if parsed.scheme != 'https' or not parsed.hostname or parsed.username or parsed.password:
            raise ValueError('HTTPS provenance without credentials required')
    grid = Grid(cells)
    original_hash = digest_file(path)
    if path.suffix == '.pbf':
        if license_name != 'ODbL-1.0':
            raise ValueError('OSM requires ODbL-1.0')
        read_osm(path, grid)
        selection = WATER_SELECTION + '; waterway=* lines; natural=coastline lines'
    else:
        read_geojson(path, grid)
        selection = 'Explicit water-area/waterway-line/coastline GeoJSON observations'
    if digest_file(path) != original_hash:
        raise ValueError('Source changed during extraction')
    source = dict(url=source_url, file_sha256=original_hash, header=grid.source_header, bytes=path.stat().st_size, license=license_name, license_url=license_url, attribution=attribution, selection=selection)
    packs, entries, total = [], [], 0
    for index, cell in enumerate(grid.cells):
        # Unlocalized source errors apply to every cell, including empty cells.
        cell.issues = grid.issues
        manifest, payload, _ = pack_bytes(cell, revision, source)
        total += len(manifest) + len(payload)
        if total > 128*1024*1024:
            raise ValueError('Batch byte limit exceeded')
        name = f'cell-{index:04d}'
        entries.append(dict(directory=name, bounds=cell.bounds, manifest_sha256=hashlib.sha256(manifest).hexdigest()))
        packs.append((name, manifest, payload))
    catalog = encoded(dict(format='rosenfu-water-evidence-catalog', version=1, revision=revision, geographic_completeness='unknown', coverage=[], source_sha256=original_hash, cells=entries))
    # All limits checked before output. Catalog is the final commit marker, pinned externally.
    output.mkdir(parents=True, exist_ok=False)
    for name, manifest, payload in packs:
        directory = output/name
        directory.mkdir()
        (directory/'evidence.json').write_bytes(payload)
        (directory/'manifest.json').write_bytes(manifest)
    (output/'catalog.json').write_bytes(catalog)
    return dict(catalog_sha256=hashlib.sha256(catalog).hexdigest(), cells=len(entries), copies=grid.copies, coordinates=grid.coordinates, source_issues=len(grid.issues), geographic_completeness='unknown')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('input', type=Path)
    parser.add_argument('--cells', type=Path, required=True, help='JSON array of [west,south,east,north], at most 256 cells')
    for option in ('revision', 'source-url', 'license', 'license-url', 'attribution', 'output'):
        parser.add_argument('--'+option, required=True)
    args = parser.parse_args()
    try:
        if args.cells.stat().st_size > MAX_BYTES:
            raise ValueError('Cell list too large')
        print(json.dumps(build_grid(args.input,json.loads(args.cells.read_text()),args.revision,args.source_url,args.license,args.license_url,args.attribution,args.output)))
    except (ValueError,OSError,KeyError,TypeError) as error:
        parser.exit(1, f'water grid: {error}\n')
