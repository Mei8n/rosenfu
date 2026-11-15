import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from grid import build_grid, Grid

class GridTests(unittest.TestCase):
    def test_shared_boundary_and_unknown_empty_cell(self):
        with tempfile.TemporaryDirectory() as root:
            p=Path(root); source=p/'source.json'
            source.write_text(json.dumps(dict(type='FeatureCollection',features=[dict(type='Feature',id='stream',properties=dict(evidence_kind='waterway-line'),geometry=dict(type='LineString',coordinates=[[134.05,34.01],[134.05,34.09]]))])))
            cells=[[134,34,134.05,34.1],[134.05,34,134.1,34.1],[135,34,135.1,34.1]]
            result=build_grid(source,cells,'fixture','https://source.test','test','https://license.test','test',p/'pack')
            catalog=(p/'pack/catalog.json').read_bytes()
            self.assertEqual(hashlib.sha256(catalog).hexdigest(),result['catalog_sha256'])
            index=json.loads(catalog); self.assertEqual(index['coverage'],[])
            counts=[]
            for entry in index['cells']:
                folder=p/'pack'/entry['directory']; raw=(folder/'manifest.json').read_bytes()
                self.assertEqual(hashlib.sha256(raw).hexdigest(),entry['manifest_sha256'])
                manifest=json.loads(raw); data=(folder/'evidence.json').read_bytes()
                self.assertEqual(hashlib.sha256(data).hexdigest(),manifest['payload']['sha256'])
                self.assertEqual(manifest['geographic_completeness'],'unknown')
                counts.append(len(json.loads(data)['observations']))
            self.assertEqual(counts,[1,1,0])
            with self.assertRaises(ValueError): build_grid(source,cells,'fixture','https://source.test','test','https://license.test','test',p/'pack')

    def test_invalid_cells(self):
        for cells in ([],[[0,0,.1,.1]]*257,[[0,0,.1,.1]]*2,[[0,0,.3,.1]]):
            with self.assertRaises(ValueError): Grid(cells)

    def test_source_issue_applies_to_all_cells(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as root:
            p=Path(root); source=p/'source.pbf'; source.write_bytes(b'fixture')
            def read(_,grid): grid.issues.append(dict(id='relation/1',reason='unresolved'))
            with patch('grid.read_osm',read):
                build_grid(source,[[0,0,.1,.1],[1,0,1.1,.1]],'fixture','https://source.test','ODbL-1.0','https://license.test','test',p/'pack')
            for file in (p/'pack').glob('cell-*/evidence.json'):
                self.assertEqual(json.loads(file.read_text())['source_issues'],[dict(id='relation/1',reason='unresolved')])
