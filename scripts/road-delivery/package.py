"""Validate generated packs and build bounded, deterministic release shards. No network."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import tempfile
import zipfile

MAX_FILE = 5 * 1024 * 1024
TARGET_EXPANDED = 32 * 1024 * 1024
MAX_EXPANDED = 256 * 1024 * 1024
MAX_COMPRESSED = 64 * 1024 * 1024
MAX_TOTAL_COMPRESSED = 2 * 1024 * 1024 * 1024
MIN_FREE = 512 * 1024 * 1024
MAX_ASSETS = 998  # national index + catalog also consume release slots


def require(ok, reason):
    if not ok:
        raise ValueError(reason)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def read(path, maximum=MAX_FILE):
    require(not path.is_symlink() and path.is_file(), 'not a regular file')
    require(path.stat().st_size <= maximum, 'file budget exceeded')
    return path.read_bytes()


def add(z, name, data):
    item = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
    item.create_system = 3
    item.compress_type = zipfile.ZIP_DEFLATED
    item.external_attr = 0o100644 << 16
    z.writestr(item, data, compresslevel=6)


def package(root, output):
    root, output = Path(root), Path(output)
    require(not root.is_symlink() and not output.exists(), 'invalid input or existing output')
    raw = read(root / 'index.json')
    index = json.loads(raw)
    require(index.get('format') == 'rosenfu-road-pack-index' and index.get('version') == 1, 'index format')
    require(index.get('license') == 'ODbL-1.0' and index.get('attribution'), 'attribution required')
    regions = index.get('regions')
    require(isinstance(regions, list) and 0 < len(regions) <= 8, 'region limit')
    output.parent.mkdir(parents=True, exist_ok=True)
    stage = Path(tempfile.mkdtemp(prefix='.road-release-', dir=output.parent))
    assets, seen = [], set()
    total_compressed = 0
    try:
        for region in regions:
            name = region['name']
            require(isinstance(name, str) and re.fullmatch('[a-z0-9-]+', name), 'region name')
            require(not (root / 'packs').is_symlink() and not (root / 'packs' / name).is_symlink(), 'symlink directory')
            for shard in region['shards']:
                path = shard['manifest']
                require(isinstance(path, str) and re.fullmatch(r'packs/' + re.escape(name) + r'/-?\d+_-?\d+/manifest\.json', path), 'manifest path')
                require(path not in seen and len(seen) < MAX_ASSETS, 'duplicate or asset limit')
                seen.add(path)
                require(shutil.disk_usage(stage).free >= MIN_FREE + MAX_EXPANDED + 16 * 1024 * 1024, 'insufficient output disk')
                directory = root / Path(path).parent
                require(not directory.is_symlink(), 'symlink directory')
                data = read(root / path)
                require(digest(data) == shard['sha256'] and len(data) == shard['bytes'], 'manifest mismatch')
                manifest = json.loads(data)
                cells = manifest['cells']
                require(isinstance(cells, dict) and len(cells) == shard['cells'] and len(cells) <= 2500, 'cell count')
                require(cells, 'empty shard')
                groups, group, size = [], [], len(data)
                for key, cell in sorted(cells.items()):
                    require(re.fullmatch(r'-?\d+_-?\d+', key) and cell['file'] == key + '.json', 'cell path')
                    require(type(cell['bytes']) is int and 0 <= cell['bytes'] <= MAX_FILE, 'cell size')
                    require(len(data) + cell['bytes'] <= MAX_EXPANDED, 'expanded cell budget exceeded')
                    if group and size + cell['bytes'] > min(TARGET_EXPANDED, MAX_EXPANDED):
                        groups.append(group); group, size = [], len(data)
                    group.append((key, cell)); size += cell['bytes']
                if group: groups.append(group)
                for number, group in enumerate(groups):
                    require(len(assets) < MAX_ASSETS, 'asset limit')
                    require(shutil.disk_usage(stage).free >= MIN_FREE + MAX_EXPANDED + 16 * 1024 * 1024, 'insufficient output disk')
                    filename = name + '-' + directory.name + f'-p{number:03d}.zip'
                    size = len(data)
                    with zipfile.ZipFile(stage / filename, 'w') as z:
                        add(z, path, data)
                        for key, cell in group:
                            content = read(directory / cell['file'])
                            require(len(content) == cell['bytes'] and digest(content) == cell['sha256'], 'cell hash mismatch')
                            size += len(content)
                            require(size <= MAX_EXPANDED, 'expanded part budget exceeded')
                            add(z, path.rsplit('/', 1)[0] + '/' + cell['file'], content)
                    archive = read(stage / filename, MAX_COMPRESSED)
                    total_compressed += len(archive)
                    require(total_compressed <= MAX_TOTAL_COMPRESSED, 'total output budget exceeded')
                    assets.append(dict(name=filename, manifest=path, manifest_sha256=shard['sha256'],
                                       sha256=digest(archive), bytes=len(archive), expanded_bytes=size,
                                       files=len(group)+1, cell_count=len(group), first_cell=group[0][0], last_cell=group[-1][0]))
        require(assets, 'no shards')
        (stage / 'index.json').write_bytes(raw)
        catalog = dict(format='rosenfu-road-release', version=2, revision=index['revision'],
                       publishable=True,
                       coverage_verified=index.get('coverage_verified') is True and index.get('polygon_snapshot_match') == 'verified',
                       coverage_note='Observed source roads only. Missing or unverified data is not proof of road absence.',
                       index_sha256=digest(raw), license=index['license'], attribution=index['attribution'],
                       license_url='https://opendatacommons.org/licenses/odbl/1-0/',
                       attribution_url='https://www.openstreetmap.org/copyright', assets=assets)
        catalog_bytes = (json.dumps(catalog, sort_keys=True, separators=(',', ':')) + '\n').encode()
        require(len(catalog_bytes) <= MAX_FILE, 'catalog size')
        (stage / 'catalog.json').write_bytes(catalog_bytes)
        stage.rename(output)
        return catalog
    finally:
        if stage.exists():
            shutil.rmtree(stage)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('input', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    print(json.dumps(package(args.input, args.output), indent=2))
