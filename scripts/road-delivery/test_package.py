import hashlib
import json
from pathlib import Path
import tempfile
import unittest
import zipfile
import package


class Packaging(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name) / 'input'
        self.shard = self.root / 'packs/test/130_30'
        self.shard.mkdir(parents=True)
        cell = b'{"ways":[]}'
        (self.shard / '6500_1500.json').write_bytes(cell)
        manifest = json.dumps({'cells': {'6500_1500': {'file': '6500_1500.json', 'bytes': len(cell), 'sha256': package.digest(cell)}}}).encode()
        (self.shard / 'manifest.json').write_bytes(manifest)
        self.index = {'format': 'rosenfu-road-pack-index', 'version': 1, 'license': 'ODbL-1.0', 'attribution': 'OSM contributors', 'revision': 'test', 'coverage_verified': False, 'regions': [{'name': 'test', 'shards': [{'manifest': 'packs/test/130_30/manifest.json', 'bytes': len(manifest), 'sha256': package.digest(manifest), 'cells': 1}]}]}
        self.write()
        self.output = Path(self.tmp.name) / 'output'

    def write(self):
        (self.root / 'index.json').write_text(json.dumps(self.index))

    def test_reproducible_distribution_and_coverage_are_distinct(self):
        result = package.package(self.root, self.output)
        self.assertTrue(result['publishable'])
        self.assertFalse(result['coverage_verified'])
        other = Path(self.tmp.name) / 'other'
        self.assertEqual(result, package.package(self.root, other))
        with zipfile.ZipFile(self.output / result['assets'][0]['name']) as archive:
            self.assertEqual(len(archive.namelist()), 2)
            self.assertIsNone(archive.testzip())

    def test_corruption_fails_atomically(self):
        (self.shard / '6500_1500.json').write_text('bad')
        with self.assertRaises(ValueError): package.package(self.root, self.output)
        self.assertFalse(self.output.exists())

    def test_path_traversal(self):
        self.index['regions'][0]['shards'][0]['manifest'] = '../manifest.json'
        self.write()
        with self.assertRaises(ValueError): package.package(self.root, self.output)

    def test_duplicate_shard(self):
        self.index['regions'][0]['shards'] *= 2
        self.write()
        with self.assertRaises(ValueError): package.package(self.root, self.output)

    def test_budget(self):
        old = package.MAX_EXPANDED
        package.MAX_EXPANDED = 1
        try:
            with self.assertRaises(ValueError): package.package(self.root, self.output)
        finally: package.MAX_EXPANDED = old

    def test_total_budget(self):
        old = package.MAX_TOTAL_COMPRESSED
        package.MAX_TOTAL_COMPRESSED = 1
        try:
            with self.assertRaises(ValueError): package.package(self.root, self.output)
        finally: package.MAX_TOTAL_COMPRESSED = old
        self.assertFalse(self.output.exists())

    def test_existing_output_preserved(self):
        self.output.mkdir()
        with self.assertRaises(ValueError): package.package(self.root, self.output)
        self.assertTrue(self.output.exists())


if __name__ == '__main__': unittest.main()
