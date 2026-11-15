#!/usr/bin/env python3
"""Import operator-verified OSM water-polygons with ODbL rights only; no other SHP sources."""
import argparse
import hashlib
import json
from pathlib import Path
from urllib.parse import urlsplit
from build import Collector, digest_file, encoded, pack_bytes


def source_files(path):
    files = [path.with_suffix(ext) for ext in ('.shp','.shx','.dbf','.prj')]
    if any(not p.is_file() for p in files) or sum(p.stat().st_size for p in files) > 8*1024**3 or files[-1].stat().st_size > 1024*1024:
        raise ValueError('Expected bounded local SHP/SHX/DBF/PRJ files')
    return [dict(file=p.name,bytes=p.stat().st_size,sha256=digest_file(p)) for p in files]


def build_marine(path, bounds, revision, source_url, output):
    import shapefile
    import shapely
    from shapely.geometry import shape, mapping, box, MultiPolygon
    from pyproj import CRS
    path, output = Path(path), Path(output)
    if output.exists() or path.suffix != '.shp':
        raise ValueError('Require .shp input and new output directory')
    if not isinstance(revision,str) or not revision or len(revision)>4096:
        raise ValueError('Invalid revision')
    parsed = urlsplit(source_url)
    if len(source_url)>4096 or parsed.scheme!='https' or not parsed.hostname or parsed.username or parsed.password:
        raise ValueError('HTTPS source URL required')
    collector = Collector(bounds)
    before = source_files(path)
    crs = CRS.from_wkt(path.with_suffix('.prj').read_text())
    if not crs.equals(CRS.from_epsg(4326), ignore_axis_order=True):
        raise ValueError('Only WGS84 longitude/latitude source accepted; no implicit reprojection')
    clip = box(*bounds)
    selected = 0
    with shapefile.Reader(str(path)) as reader:
        if reader.shapeType != shapefile.POLYGON:
            raise ValueError('Only 2D polygon Shapefile supported')
        for record in reader.iterShapes(bbox=bounds):
            selected += 1
            if selected > 10000 or len(record.points)>100000:
                raise ValueError('Marine source geometry limit; preprocess a smaller source')
            for index, start in enumerate(record.parts):
                end = record.parts[index+1] if index+1<len(record.parts) else len(record.points)
                ring = record.points[start:end]
                if len(ring)<4 or ring[0]!=ring[-1] or any(abs(a[0]-b[0])>180 for a,b in zip(ring,ring[1:])):
                    raise ValueError('Unclosed or antimeridian-crossing marine ring')
            geojson = record.__geo_interface__
            # Pinned pyshp 2.3.1 records orphan/all-hole promotion here; never accept it.
            if record._errors:
                raise ValueError('Shapefile ring conversion requires repair')
            geometry = shape(geojson)
            if geometry.is_empty or geometry.geom_type not in ('Polygon','MultiPolygon') or not geometry.is_valid:
                raise ValueError('Invalid marine polygon; no automatic repair permitted')
            if any(abs(x)>180 or abs(y)>90 for x,y in record.points):
                raise ValueError('Invalid geographic source coordinates')
            clipped = geometry.intersection(clip)
            if clipped.is_empty:
                continue
            # A boundary-only intersection has no area. Never turn it into water width.
            polygons = []
            def collect(g):
                if g.geom_type == 'Polygon': polygons.append(g)
                elif g.geom_type in ('MultiPolygon','GeometryCollection'):
                    for part in g.geoms: collect(part)
            collect(clipped)
            if not polygons:
                continue
            result = polygons[0] if len(polygons)==1 else MultiPolygon(polygons)
            if not result.is_valid:
                raise ValueError('Invalid clipped geometry')
            # JSON normalizes Shapely tuples to the reader/builder's list contract.
            collector.add(f'marine-record/{record.oid}', 'water-area', json.loads(json.dumps(mapping(result))), {'source_dataset':'marine-water-polygons','geometry_processing':'WGS84 planar intersection with requested bounds; no simplification'})
    if source_files(path) != before:
        raise ValueError('Marine source changed during extraction')
    bundle_hash = hashlib.sha256(encoded(before)).hexdigest()
    source = dict(url=source_url, file_sha256=bundle_hash, bytes=sum(f['bytes'] for f in before), files=before,
        hash_scope='SHA256 of canonical component inventory, not ZIP or single SHP bytes',
        license='ODbL-1.0',license_url='https://opendatacommons.org/licenses/odbl/1-0/', attribution='© OpenStreetMap contributors; marine polygons extracted by Rosenfu',
        selection='Operator-identified OSM marine polygon dataset; source bbox selection, then planar intersection',
        processing=dict(shapely=shapely.__version__,geos=shapely.geos_version_string,crs='EPSG:4326',selected_records=selected))
    _, payload, manifest = pack_bytes(collector,revision,source)
    manifest['limitations'] = ['No absence-of-water or dry-land certification','Marine polygons only; lakes and narrow rivers remain unresolved','Geometry clipped to requested bounds in WGS84 planar coordinates; no simplification, buffering, union or automatic repair','Split source polygon overlap retained; do not sum overlapping lengths or areas','Source completeness, actual dataset identity and date not independently guaranteed']
    manifest_bytes = encoded(manifest)
    output.mkdir(parents=True,exist_ok=False)
    (output/'evidence.json').write_bytes(payload)
    (output/'manifest.json').write_bytes(manifest_bytes)
    return dict(manifest_sha256=hashlib.sha256(manifest_bytes).hexdigest(), payload_sha256=manifest['payload']['sha256'], counts=manifest['payload']['counts'], source_bundle_sha256=bundle_hash, geographic_completeness='unknown')


if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('input',type=Path)
    parser.add_argument('--bounds',nargs=4,type=float,required=True)
    for option in ('revision','source-url','output'): parser.add_argument('--'+option,required=True)
    args=parser.parse_args()
    try:
        print(json.dumps(build_marine(args.input,args.bounds,args.revision,args.source_url,args.output)))
    except (ValueError,OSError,KeyError,TypeError,ImportError) as error:
        parser.exit(1,f'marine pack: {error}\n')
