#!/usr/bin/env python3
"""Build an observations-only fine raster from a pinned Rosenfu composite.
Requires numpy + shapely. Does not certify model completeness or publish data.
"""
import argparse
import hashlib
import json
import math
import re
import struct
from pathlib import Path

MAX_FILE = 32 * 1024 * 1024
MAX_CELLS = 2_000_000
POLICY = 'observed-surface-centres-v2'

def sha(data):
    return hashlib.sha256(data).hexdigest()

def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()

def read(root, name, pin):
    rel = Path(name)
    if rel.is_absolute() or '..' in rel.parts or not rel.parts:
        raise ValueError('Invalid input path')
    path = root / rel
    if not path.resolve().is_relative_to(root.resolve()) or path.stat().st_size > MAX_FILE:
        raise ValueError('Input path/size limit')
    data = path.read_bytes()
    if len(data) > MAX_FILE or sha(data) != pin:
        raise ValueError('Input hash/size mismatch')
    return json.loads(data)

def build(root, pin, out, cell=25.):
    import numpy as np
    from shapely.geometry import shape
    from shapely import union_all, intersects_xy
    if out.exists():
        raise ValueError('Output already exists')
    if not math.isfinite(cell) or not 1 <= cell <= 100:
        raise ValueError('Invalid cell size')
    composite = read(root, 'composite.json', pin)
    if composite.get('format') != 'rosenfu-water-composite' or composite.get('version') != 1:
        raise ValueError('Unsupported composite')
    parts = composite['parts']
    if not 1 <= len(parts) <= 32 or len({p['key'] for p in parts}) != len(parts):
        raise ValueError('Invalid parts')
    geometries, sources, issues, bounds = [], [], [], None
    excluded = non_area = coordinate_count = 0
    for part in parts:
        m = read(root, part['manifest'], part['manifest_sha256'])
        p = read(root, part['payload'], m['payload']['sha256'])
        if (root/part['payload']).stat().st_size != m['payload']['bytes']:
            raise ValueError('Payload byte count mismatch')
        if m.get('format') != 'rosenfu-water-evidence-manifest' or p.get('format') != 'rosenfu-water-evidence' or m.get('version') != 1 or p.get('version') != 1:
            raise ValueError('Unsupported evidence')
        if bounds is None:
            bounds = m['bounds']
        if m['bounds'] != bounds or p['bounds'] != bounds or p['revision'] != m['revision']:
            raise ValueError('Evidence bounds/revision mismatch')
        source = m['source']
        if not isinstance(m['revision'],str) or not 1 <= len(m['revision']) <= 4096 or not isinstance(source.get('file_sha256'),str) or not re.fullmatch('[0-9a-f]{64}',source['file_sha256']):
            raise ValueError('Invalid source revision/hash')
        if any(not isinstance(source.get(k),str) or not 1 <= len(source[k]) <= 4096 for k in ['url','license','attribution']) or not source['url'].startswith('https://'):
            raise ValueError('Invalid source attribution')
        sources.append(dict(key=part['key'], revision=m['revision'], url=source['url'], source_sha256=source['file_sha256'], license=source['license'], attribution=source['attribution']))
        if not isinstance(p['source_issues'],list) or len(encoded(p['source_issues'])) > 1_000_000:
            raise ValueError('Source issue size limit')
        issues.extend(dict(source=part['key'], issue=i) for i in p['source_issues'])
        if len(issues) > 1000 or len(p['observations']) > 100000:
            raise ValueError('Observation/issue limit')
        for o in p['observations']:
            if o['kind'] != 'water-area':
                non_area += 1
                continue
            t = o['tags']
            if any(k in t and t[k] != 'no' for k in ['covered', 'tunnel', 'bridge', 'intermittent', 'seasonal']) or any(k in t and t[k] != '0' for k in ['layer', 'level', 'ele']) or t.get('location') in ['underground', 'underwater'] or t.get('landuse') == 'basin' or t.get('water') == 'basin' or 'basin' in t:
                excluded += 1
                continue
            g = shape(o['geometry'])
            if g.geom_type not in ['Polygon', 'MultiPolygon'] or not g.is_valid or g.is_empty or g.has_z or g.has_m:
                raise ValueError('Invalid observed polygon')
            from shapely import get_num_coordinates
            coordinate_count += int(get_num_coordinates(g))
            if coordinate_count > 1_000_000:
                raise ValueError('Coordinate limit')
            geometries.append(g)
    if not isinstance(bounds,list) or len(bounds)!=4 or any(type(x) not in [int,float] for x in bounds):
        raise ValueError('Invalid bounds')
    w, s, e, n = bounds
    if not all(math.isfinite(x) for x in bounds) or not -180 <= w < e <= 180 or not -85 <= s < n <= 85 or e-w > 180:
        raise ValueError('Unsupported bounds')
    lon, lat = (w+e)/2, (s+n)/2
    rad = math.pi/180
    k, ky = 6371008.8*rad*math.cos(lat*rad), 6371008.8*rad
    x0, y0 = math.floor((w-lon)*k/cell)*cell, math.floor((s-lat)*ky/cell)*cell
    width, height = math.ceil(((e-lon)*k-x0)/cell), math.ceil(((n-lat)*ky-y0)/cell)
    if width > 65535 or height > 65535 or width*height > MAX_CELLS:
        raise ValueError('Raster limit')
    if not -180 <= lon+x0/k < lon+(x0+width*cell)/k <= 180 or not -85 <= lat+y0/ky < lat+(y0+height*cell)/ky <= 85:
        raise ValueError('Rounded raster extent unsupported')
    xx, yy = lon+(x0+(np.arange(width)+.5)*cell)/k, lat+(y0+(np.arange(height)+.5)*cell)/ky
    wet = intersects_xy(union_all(geometries), xx[None, :], yy[:, None])
    # Cells outside the requested evidence box must never acquire water through
    # source geometry that extends beyond its declared footprint.
    wet &= (xx[None, :] >= w) & (xx[None, :] < e) & (yy[:, None] >= s) & (yy[:, None] < n)
    rows, edges = [0], []
    for row in wet:
        edges.extend(np.flatnonzero(np.diff(np.r_[False, row, False])).tolist())
        rows.append(len(edges))
    payload = struct.pack('<'+'I'*len(rows), *rows)+struct.pack('<'+'H'*len(edges), *edges)
    if len(payload) > 16*1024*1024:
        raise ValueError('Payload limit')
    # Same source set + interpretation rule => same revision across tile bounds.
    revision = sha(encoded(dict(policy=POLICY, sources=sorted(sources, key=lambda x:x['key']))))
    spec = dict(id='observations-'+pin, revision=revision, coverage='observations_only', originLongitude=lon, originLatitude=lat, x0=x0, y0=y0, cellM=cell, width=width, height=height)
    manifest = dict(format='rosenfu-water-raster', version=1, spec=spec, source_composite_sha256=pin, sources=sources, source_issues=issues, bounds=bounds, coverage=[], geographic_completeness='unknown', excluded_area_observations=excluded, non_area_observations=non_area, selected_area_observations=len(geometries), water_cells=int(wet.sum()), unknown_cells=int((~wet).sum()), payload=dict(file='water.bin', bytes=len(payload), sha256=sha(payload), row_bytes=len(rows)*4))
    # Publish the manifest last; partial output can never look like a complete pack.
    out.mkdir(parents=True)
    (out/'water.bin').write_bytes(payload)
    (out/'manifest.json').write_bytes(encoded(manifest)+b'\n')
    return manifest

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('input', type=Path)
    parser.add_argument('--pin', required=True)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--cell', default=25., type=float)
    a = parser.parse_args()
    result = build(a.input, a.pin, a.output, a.cell)
    print(json.dumps({k:result[k] for k in ['spec','water_cells','unknown_cells','payload']}))
