import bz2
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from build import generate, covered, polygon

XML = '''<osm version="0.6"><bounds minlat="0" minlon="0" maxlat="1" maxlon="1"/>
<node id="90071992547409931" lon="0.005" lat="0.01"/>
<node id="2" lon="0.035" lat="0.01"/>
<way id="90071992547409933" version="4"><nd ref="90071992547409931"/><nd ref="2"/>
<tag k="highway" v="service"/><tag k="bridge" v="no"/><tag k="tunnel" v="yes"/>
<tag k="layer" v="-1"/><tag k="level" v="-0.5"/></way></osm>'''
COVER = {'type':'Polygon','coordinates':[[[0,0],[.06,0],[.06,.02],[0,.02],[0,0]]]}


class PackTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.source = self.base/'in.osm'
        self.source.write_text(XML)
        self.coverage = self.base/'coverage.json'
        self.coverage.write_text(json.dumps(COVER))
        self.output = self.base/'pack'

    def run_pack(self, **kwargs):
        return generate(self.source,self.coverage,(0,0,.06,.02),'test-revision','fixture:osm',self.output,**kwargs)

    def test_cross_cell_full_way_empty_and_hash(self):
        manifest = self.run_pack()
        self.assertEqual(len(manifest['cells']),3)
        a = json.loads((self.output/'0_0.json').read_text())
        b = json.loads((self.output/'1_0.json').read_text())
        self.assertEqual(a['ways'],b['ways'])
        way = a['ways'][0]
        self.assertEqual(way['id'],'90071992547409933')
        self.assertEqual(way['node_ids'],['90071992547409931','2'])
        self.assertEqual(way['coordinates'],[[.005,.01],[.035,.01]])
        self.assertEqual([way['tags'][k] for k in ('bridge','tunnel','layer','level')],['no','yes','-1','-0.5'])
        self.assertEqual(manifest['cells']['2_0']['ways'],0)
        for item in manifest['cells'].values():
            data = (self.output/item['file']).read_bytes()
            self.assertEqual(hashlib.sha256(data).hexdigest(),item['sha256'])
        self.assertEqual(manifest['source']['coverage_geojson'],COVER)

    def test_missing_node_fails_without_output(self):
        self.source.write_text(XML.replace('<node id="2" lon="0.035" lat="0.01"/>',''))
        with self.assertRaisesRegex(ValueError,'missing referenced'):
            self.run_pack()
        self.assertFalse(self.output.exists())

    def test_xml_bounds_do_not_certify_coverage(self):
        self.coverage.write_text(json.dumps({'type':'Polygon','coordinates':[[[0,0],[.04,0],[.04,.02],[0,.02],[0,0]]]}))
        with self.assertRaisesRegex(ValueError,'certified coverage'):
            self.run_pack()
        self.assertFalse(self.output.exists())

    def test_concave_notch_rejects_even_with_all_corners_inside(self):
        ring = polygon({'type':'Polygon','coordinates':[[[0,0],[2,0],[2,2],[1.2,2],[1.2,.5],[.8,.5],[.8,2],[0,2],[0,0]]]})
        self.assertFalse(covered((.1,.1,1.9,1.9),ring))

    def test_bz2_and_expanded_limit(self):
        self.source = self.base/'in.osm.bz2'
        self.source.write_bytes(bz2.compress(XML.encode()))
        with self.assertRaisesRegex(ValueError,'expanded input'):
            self.run_pack(limits={'input_bytes':len(XML.encode())-1})
        self.assertEqual(self.run_pack()['source']['xml_sha256'],hashlib.sha256(XML.encode()).hexdigest())

    def test_limits(self):
        for limits in ({'cells':2},{'features':1},{'coords':3},{'input_bytes':1}):
            with self.subTest(limits=limits), self.assertRaises(ValueError):
                self.run_pack(limits=limits)
            self.assertFalse(self.output.exists())

    def test_dtd_rejected(self):
        self.source.write_text('<!DOCTYPE osm [<!ENTITY a "x">]>'+XML)
        with self.assertRaisesRegex(ValueError,'DTD'):
            self.run_pack()

    def test_filter_area_and_unselected_class(self):
        self.source.write_text(XML.replace('v="service"','v="footway"'))
        self.assertTrue(all(c['ways']==0 for c in self.run_pack()['cells'].values()))

    def test_invalid_polygon_and_holes(self):
        for coords in ([[[0,0],[2,2],[0,2],[2,0],[0,0]]], COVER['coordinates']*2):
            with self.assertRaises(ValueError):
                polygon({'type':'Polygon','coordinates':coords})

    def test_segment_crosses_cell_without_inside_nodes(self):
        self.source.write_text(XML.replace('lon="0.005"','lon="-0.01"').replace('lon="0.035"','lon="0.07"'))
        manifest = self.run_pack()
        self.assertTrue(all(c['ways']==1 for c in manifest['cells'].values()))

    def test_boundary_way_is_in_both_cells(self):
        self.source.write_text(XML.replace('lon="0.005" lat="0.01"','lon="0.02" lat="0.005"').replace('lon="0.035" lat="0.01"','lon="0.02" lat="0.015"'))
        manifest = self.run_pack()
        self.assertEqual(manifest['cells']['0_0']['ways'],1)
        self.assertEqual(manifest['cells']['1_0']['ways'],1)

    def test_area_way_excluded(self):
        self.source.write_text(XML.replace('</way>', '<tag k="area" v="yes"/></way>'))
        self.assertTrue(all(c['ways']==0 for c in self.run_pack()['cells'].values()))

    def test_no_clobber(self):
        self.run_pack()
        before = (self.output/'manifest.json').read_bytes()
        with self.assertRaisesRegex(ValueError,'must not exist'):
            self.run_pack()
        self.assertEqual(before,(self.output/'manifest.json').read_bytes())


if __name__ == '__main__':
    unittest.main()
