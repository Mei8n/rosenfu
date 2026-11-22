#!/usr/bin/env python3
"""Build source-relative, unsimplified OSM road cells. No network or dependencies."""
import argparse
import bz2
import hashlib
import json
import math
from pathlib import Path
import os
import shutil
import tempfile
import xml.etree.ElementTree as ET

CLASSES = 'motorway motorway_link trunk trunk_link primary primary_link secondary secondary_link tertiary tertiary_link unclassified residential living_street service'.split()
TAGS = ('highway', 'bridge', 'tunnel', 'layer', 'level', 'access', 'motor_vehicle')
LIMITS = dict(input_bytes=128 * 1024 * 1024, cells=4096, features=100000, coords=1000000)
STEP = .02


def require(ok, message):
    if not ok:
        raise ValueError(message)


def encoded(value):
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'), allow_nan=False) + '\n').encode()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read_limited(path, limit, compressed=False):
    require(path.stat().st_size <= limit, 'input bytes limit')
    opener = bz2.open if compressed else open
    with opener(path, 'rb') as stream:
        data = stream.read(limit + 1)
    require(len(data) <= limit, 'expanded input bytes limit')
    return data


def point(p):
    require(isinstance(p, (list, tuple)) and len(p) == 2, 'invalid coordinate')
    require(all(type(v) in (int, float) and math.isfinite(v) for v in p), 'nonfinite coordinate')
    require(-180 <= p[0] <= 180 and -85 <= p[1] <= 85, 'coordinate outside supported domain')
    return tuple(p)


def cross(a, b, c):
    return (b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0])


def on(a, b, p):
    return cross(a, b, p) == 0 and all(min(a[i], b[i]) <= p[i] <= max(a[i], b[i]) for i in (0, 1))


def intersects(a, b, c, d):
    v = (cross(a,b,c), cross(a,b,d), cross(c,d,a), cross(c,d,b))
    return (v[0]*v[1] < 0 and v[2]*v[3] < 0) or any((on(a,b,c), on(a,b,d), on(c,d,a), on(c,d,b)))


def polygon(value):
    if value.get('type') == 'Feature':
        value = value.get('geometry', {})
    require(value.get('type') == 'Polygon', 'coverage must be one Polygon')
    rings = value.get('coordinates', [])
    require(len(rings) == 1, 'holes unsupported: split into independently certified packs')
    ring = [point(p) for p in rings[0]]
    require(4 <= len(ring) <= 513 and ring[0] == ring[-1], 'closed polygon of 3..512 vertices required')
    edges = list(zip(ring, ring[1:]))
    for i, (a,b) in enumerate(edges):
        require(a != b, 'zero polygon edge')
        c = edges[(i+1) % len(edges)][1]
        require(not (cross(a,b,c) == 0 and sum((a[k]-b[k])*(c[k]-b[k]) for k in (0,1)) > 0), 'overlapping adjacent polygon edges')
        for j, (c,d) in enumerate(edges):
            if j <= i+1 or (i == 0 and j == len(edges)-1):
                continue
            require(not intersects(a,b,c,d), 'self-intersecting polygon')
    require(sum(a[0]*b[1]-b[0]*a[1] for a,b in edges) != 0, 'degenerate polygon')
    return ring


def inside(p, ring):
    hit = False
    for a,b in zip(ring, ring[1:]):
        if on(a,b,p):
            return True
        if (a[1] > p[1]) != (b[1] > p[1]):
            if p[0] < (b[0]-a[0])*(p[1]-a[1])/(b[1]-a[1])+a[0]:
                hit = not hit
    return hit


def clipped(a, b, box):
    lo, hi = 0., 1.
    for axis, lower, upper in ((0, box[0], box[2]), (1, box[1], box[3])):
        delta = b[axis]-a[axis]
        if delta == 0:
            if not lower <= a[axis] <= upper:
                return None
        else:
            u,v = sorted(((lower-a[axis])/delta, (upper-a[axis])/delta))
            lo,hi = max(lo,u),min(hi,v)
    return (lo,hi) if lo <= hi else None


def covered(box, ring):
    w,s,e,n = box
    if not all(inside(p, ring) for p in ((w,s),(e,s),(e,n),(w,n))):
        return False
    # Corners alone miss concave notches: no boundary may pass through cell interior.
    for a,b in zip(ring, ring[1:]):
        interval = clipped(a,b,box)
        if interval:
            t = sum(interval)/2
            x,y = (a[i]+t*(b[i]-a[i]) for i in (0,1))
            if w < x < e and s < y < n:
                return False
    return True


def osm_id(value):
    require(value is not None and value.isascii() and value.isdigit() and 0 < int(value) < 2**63, 'invalid OSM ID')
    return str(int(value))


def grid_cells(bounds, max_cells=4096):
    w,s,e,n=bounds
    point((w,s));point((e,n));require(w<e and s<n,'invalid bounds')
    x0,x1=math.floor(w/STEP),math.ceil(e/STEP)
    y0,y1=math.floor(s/STEP),math.ceil(n/STEP)
    require((x1-x0)*(y1-y0)<=max_cells,'cell limit')
    return [(f'{x}_{y}',tuple(round(v*STEP,10) for v in (x,y,x+1,y+1)))
            for x in range(x0,x1) for y in range(y0,y1)]


def read_osm_xml(source, limit, expected_sha=None):
    raw = read_limited(source, limit)
    if expected_sha is not None:
        require(source.suffix != ".bz2" and sha(raw) == expected_sha, "raw OSM source digest mismatch")
    xml = read_limited(source, limit, True) if source.suffix == '.bz2' else raw
    # Reject DTD/entities and non-UTF8 XML before ElementTree can expand anything.
    decoded = xml.decode('utf-8-sig')
    require('<!DOCTYPE' not in decoded.upper() and '<!ENTITY' not in decoded.upper(), 'DTD/entities prohibited')
    root = ET.fromstring(decoded)
    require(root.tag == 'osm', 'OSM XML required')
    return raw, xml, root


def read_tags(element):
    tags = {}
    for tag in element.findall('tag'):
        k, v = tag.get('k'), tag.get('v')
        require(k is not None and v is not None and k not in tags and len(k)<=256 and len(v)<=4096, 'invalid tags')
        tags[k] = v
    return tags


def generate(source, coverage, bounds, revision, origin, output, limits=None):
    limits = {**LIMITS, **(limits or {})}
    require(all(type(v) is int and v > 0 for v in limits.values()), 'positive limits required')
    require(revision and origin and len(revision) <= 256 and len(origin) <= 4096, 'source and revision required')
    require(not output.exists(), 'output must not exist')
    w,s,e,n = bounds
    point((w,s)); point((e,n))
    require(w < e and s < n, 'invalid bounds')
    coverage_bytes = read_limited(coverage, 1024*1024)
    coverage_json = json.loads(coverage_bytes)
    ring = polygon(coverage_json)
    cells = {}
    for key,box in grid_cells(bounds,limits['cells']):
        require(covered(box,ring), 'cell not wholly inside certified coverage polygon')
        cells[key] = {'bounds':list(box),'ways':[]}
    raw, xml, root = read_osm_xml(source, limits['input_bytes'])
    nodes, ways, seen_ways = {}, [], set()
    for el in root:
        if el.tag == 'node':
            ident = osm_id(el.get('id'))
            require(ident not in nodes, 'duplicate node')
            nodes[ident] = point((float(el.attrib['lon']),float(el.attrib['lat'])))
            require(len(nodes) <= limits['coords'], 'node coordinate limit')
        elif el.tag == 'way':
            ident = osm_id(el.get('id'))
            require(ident not in seen_ways, 'duplicate way')
            seen_ways.add(ident)
            require(len(seen_ways) <= limits['features'], 'feature limit')
            tags = read_tags(el)
            if tags.get('highway') not in CLASSES or tags.get('area') == 'yes':
                continue
            refs = [osm_id(nd.get('ref')) for nd in el.findall('nd')]
            require(len(refs)>=2, 'road needs two nodes')
            ways.append({'id':ident, 'version':el.get('version'), 'node_ids':refs, 'tags':{k:tags.get(k) for k in TAGS}})
    coordinate_count = emitted_coords = emitted_features = 0
    for way in ways:
        require(all(ref in nodes for ref in way['node_ids']), 'missing referenced road node')
        points = [nodes[ref] for ref in way['node_ids']]
        coordinate_count += len(points)
        require(coordinate_count <= limits['coords'], 'road coordinate limit')
        way['coordinates'] = points
        for cell in cells.values():
            if any(clipped(a,b,cell['bounds']) is not None for a,b in zip(points,points[1:])):
                emitted_features += 1
                emitted_coords += len(points)
                require(emitted_features <= limits['features'] and emitted_coords <= limits['coords'], 'duplicated output limit')
                cell['ways'].append(way)
    manifest = {'format':'rosenfu-road-pack','version':1,'revision':revision,'cell_degrees':STEP,
        'source':{'url':origin,'file_sha256':sha(raw),'xml_sha256':sha(xml),'coverage_geojson':coverage_json,'coverage_sha256':sha(coverage_bytes)},
        'coverage_meaning':'Complete extraction of selected roads from the operator-certified source snapshot; not real-world completeness.',
        'requested_bounds':list(bounds),'highway_classes':CLASSES,'excluded_area':'yes','license':'ODbL-1.0',
        'license_url':'https://opendatacommons.org/licenses/odbl/1-0/','attribution':'© OpenStreetMap contributors',
        'attribution_url':'https://www.openstreetmap.org/copyright','limits':limits,'cells':{}}
    output.parent.mkdir(parents=True, exist_ok=True)
    temp = Path(tempfile.mkdtemp(prefix='.road-pack-', dir=output.parent))
    try:
        for key,cell in sorted(cells.items()):
            cell['ways'].sort(key=lambda way:int(way['id']))
            data = encoded({'format':'rosenfu-road-cell','version':1,'revision':revision,**cell})
            (temp/f'{key}.json').write_bytes(data)
            manifest['cells'][key] = {'file':f'{key}.json','bounds':cell['bounds'],'sha256':sha(data),'bytes':len(data),'ways':len(cell['ways'])}
        (temp/'manifest.json').write_bytes(encoded(manifest))
        require(not output.exists(), 'output appeared during generation')
        os.rename(temp, output)
    finally:
        if temp.exists():
            shutil.rmtree(temp)
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('--coverage', type=Path, required=True)
    parser.add_argument('--bounds', type=float, nargs=4, metavar=('WEST','SOUTH','EAST','NORTH'), required=True)
    parser.add_argument('--revision', required=True)
    parser.add_argument('--source-url', required=True)
    parser.add_argument('--output', type=Path, required=True)
    for name,value in LIMITS.items():
        parser.add_argument('--max-'+name.replace('_','-'), type=int, default=value)
    args = parser.parse_args()
    try:
        generate(args.source,args.coverage,args.bounds,args.revision,args.source_url,args.output,
                 {name:getattr(args,'max_'+name) for name in LIMITS})
    except (ValueError, OSError, KeyError, ET.ParseError) as exc:
        parser.exit(1, f'road-pack: {exc}\n')


if __name__ == '__main__':
    main()
