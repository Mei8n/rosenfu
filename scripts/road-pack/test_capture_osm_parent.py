from pathlib import Path
import json
import os
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET

import build
import build_native_region as native
import capture_osm_parent as collector
from capture_osm_parent import capture, digest, encode, osmium, publish_stage, region

HALO=region('0,0,0.01,0.01')
INNER=region('0.002,0.002,0.008,0.008')
REV='2026-11-22T12:00:00Z'


@unittest.skipUnless(osmium is not None, "pinned optional pyosmium required")
class CaptureTest(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory()
        self.root=Path(self.tmp.name)
        self.addCleanup(self.tmp.cleanup)

    def pbf(self, name, *, omit_member=False, revision=REV):
        p=self.root/name
        h=osmium.io.Header()
        if revision:
            h.set('osmosis_replication_timestamp',revision)
        with osmium.SimpleWriter(str(p),header=h) as w:
            for i,pnt in enumerate([
                    (0.003,0.003),(0.004,0.004),(0.020,0.005),(0.030,0.006),
                    (-0.03,0.002),(0.05,0.004),(0.025,0.004)],1):
                w.add_node(osmium.osm.mutable.Node(id=i,location=pnt,version=1,uid=987,
                                                  user='synthetic editor marker',changeset=654))
            for i,refs in [(10,[1,2,3]),(11,[3,4]),(12,[5,6]),(13,[4,7])]:
                if omit_member and i==11:
                    continue
                w.add_way(osmium.osm.mutable.Way(id=i,nodes=refs,version=1,tags={'highway':'unclassified'}))
            w.add_relation(osmium.osm.mutable.Relation(id=21,version=1,
                members=[('w',10,'from'),('n',3,'via'),('w',11,'to')],
                tags={'type':'restriction','restriction:bus:conditional':'no_right_turn @ (Mo-Fr)'}))
            w.add_relation(osmium.osm.mutable.Relation(id=22,version=1,
                members=[('r',23,'via'),('w',13,'to')],tags={'type':'restriction:bus'}))
            w.add_relation(osmium.osm.mutable.Relation(id=23,version=1,
                members=[('r',24,'')],tags={'type':'route'}))
            w.add_relation(osmium.osm.mutable.Relation(id=24,version=1,
                members=[('n',2,'')],tags={'type':'route'}))
        return p

    def test_exact_halo_restrictions_and_nonclaims(self):
        parent=self.pbf('parent.osm.pbf')
        data,r=capture(parent,digest(parent),HALO,INNER)
        self.assertEqual(r['revision'],REV)
        self.assertEqual(r['halo_seed_nodes'],2)
        self.assertEqual(r['halo_incident_ways'],1)
        self.assertEqual(r['observed_restriction_ids'],[21,22])
        self.assertEqual(set(data['way']),{10,11,13})
        self.assertEqual(set(data['relation']),{21,22,23,24})
        self.assertEqual(set(data['node']),{1,2,3,4,7})
        self.assertFalse(r['can_emit_gameplay_certificate'])
        self.assertNotIn(12,data['way'])  # crossing with no halo vertex is not claimed
        raw=encode(data,r)
        for forbidden in (b'uid=',b'changeset=',b'user=',b'synthetic editor marker'):
            self.assertNotIn(forbidden,raw)
        stage=self.root/'stage.osm'
        stage.write_bytes(raw)
        parsed=ET.fromstring(raw)
        self.assertEqual(parsed.find('meta').get('osm_base'),REV)
        self.assertEqual(parsed.find("relation[@id='21']/tag[@k='restriction:bus:conditional']").get('v'),
                         'no_right_turn @ (Mo-Fr)')
        self.assertEqual(parsed.find("relation[@id='22']/member").get('ref'),'23')
        with self.assertRaisesRegex(ValueError,'unsupported restriction member'):
            native.parse_source(stage,digest(stage),REV)  # unresolved semantics cannot become movement

    def test_missing_same_parent_member_detected(self):
        parent=self.pbf('incomplete.osm.pbf',omit_member=True)
        _,r=capture(parent,digest(parent),HALO,INNER)
        self.assertEqual(r['missing_selected_references']['way'],[11])
        self.assertFalse(r['can_emit_gameplay_certificate'])

    def test_rejects_missing_replication_timestamp_or_unpinned_parent(self):
        parent=self.pbf('untimed.osm.pbf',revision=None)
        with self.assertRaisesRegex(ValueError,'replication timestamp'):
            capture(parent,digest(parent),HALO,INNER)
        with self.assertRaisesRegex(ValueError,'digest mismatch'):
            capture(parent,'0'*64,HALO,INNER)


    def test_detects_parent_change_even_if_content_digest_returns_to_same_value(self):
        parent=self.pbf('mutable.osm.pbf')
        expected=digest(parent)
        original=collector.scan
        def scan_and_touch(path, kind, callback):
            original(path, kind, callback)
            info=path.stat()
            os.utime(path,ns=(info.st_atime_ns,info.st_mtime_ns+2_000_000_000))
        with patch.object(collector,'scan',side_effect=scan_and_touch):
            with self.assertRaisesRegex(ValueError,'parent changed'):
                capture(parent,expected,HALO,INNER)

    def test_publication_commit_marker_and_no_clobber(self):
        destination=self.root/'stage'
        receipt={'format':1}
        original=os.link
        def fail_on_receipt(source, target):
            if target.name=='receipt.json':
                raise OSError('synthetic storage error')
            return original(source,target)
        with patch.object(collector.os,'link',side_effect=fail_on_receipt):
            with self.assertRaisesRegex(OSError,'synthetic storage error'):
                publish_stage(destination,b'<osm/>',receipt)
        self.assertFalse(destination.exists())
        self.assertEqual(list(self.root.iterdir()),[])
        marker=publish_stage(destination,b'<osm/>',receipt)
        self.assertEqual(marker['stage_sha256'],build.sha(b'<osm/>'))
        self.assertEqual(json.loads((destination/'receipt.json').read_bytes()),marker)
        with self.assertRaises(FileExistsError):
            publish_stage(destination,b'other',receipt)
        self.assertEqual((destination/'halo.staging.osm').read_bytes(),b'<osm/>')

    def test_cli_stage_is_consumed_directly_by_existing_native_generator(self):
        script_dir=Path(__file__).parent
        parent=self.root/'shared-parent.osm.pbf'
        header=osmium.io.Header()
        header.set('osmosis_replication_timestamp',REV)
        with osmium.SimpleWriter(str(parent),header=header) as writer:
            for index,(x,y) in enumerate([(139.004,35.005),(139.005,35.005),(139.006,35.005)],1):
                writer.add_node(osmium.osm.mutable.Node(id=index,version=1,location=(x,y)))
            writer.add_way(osmium.osm.mutable.Way(
                id=10,version=1,nodes=(1,2,3),tags={'highway':'residential','bus':'yes','oneway':'yes'}))
        stage_dir=self.root/'private-stage'
        capture_cmd=[sys.executable,str(script_dir/'capture_osm_parent.py'),str(parent),
                     '--sha256',digest(parent),'--halo','139,35,139.02,35.02',
                     '--inner','139.001,35.001,139.015,35.015','--out-dir',str(stage_dir)]
        output=subprocess.run(capture_cmd,check=True,capture_output=True)
        receipt=json.loads(output.stdout)
        self.assertFalse(receipt['source_database_completeness_attested'])
        self.assertFalse(receipt['can_emit_gameplay_certificate'])
        self.assertEqual(set(p.name for p in stage_dir.iterdir()),{'halo.staging.osm','receipt.json'})
        self.assertEqual(receipt,json.loads((stage_dir/'receipt.json').read_bytes()))
        duplicate=subprocess.run(capture_cmd,capture_output=True)
        self.assertEqual(duplicate.returncode,2)
        staged=stage_dir/'halo.staging.osm'
        self.assertEqual(receipt['stage_sha256'],digest(staged))
        coverage=self.root/'independent-fixture-envelope.json'
        coverage.write_bytes(build.encoded({'type':'Polygon','coordinates':[[
            [139,35],[139.02,35],[139.02,35.02],[139,35.02],[139,35],
        ]]}))
        policy=self.root/'synthetic-explicit-only-policy.json'
        policy.write_bytes(build.encoded({
            'format':1,'asset_id':'synthetic-capture','source_sha256':receipt['stage_sha256'],
            'coverage_sha256':digest(coverage),'revision':receipt['revision'],'origin':'Synthetic fixture',
            'region':'Synthetic verified-for-test origin','bounds':receipt['inner'],
            'modes':['bus'],'turns':[],'bus_stop_positions':[],'allows_player_bus_stops':True,
        }))
        asset_path=self.root/'candidate.json'
        projection=subprocess.run([sys.executable,str(script_dir/'build_native_region.py'),str(staged),
            '--coverage',str(coverage),'--policy',str(policy),'--output',str(asset_path)],
            check=True,capture_output=True)
        result=json.loads(projection.stdout)
        self.assertTrue(result['candidate_only'])
        self.assertEqual(result['source_sha256'],receipt['stage_sha256'])
        asset=json.loads(asset_path.read_bytes())
        self.assertEqual([edge['id'] for edge in asset['edges']],['osm:10:0:forward','osm:10:1:forward'])
        self.assertEqual(asset['transitions'],[{'mode':'bus','incoming':'osm:10:0:forward','outgoing':'osm:10:1:forward'}])
        self.assertTrue(asset['allows_player_bus_stops'])
        self.assertEqual(asset['bus_stop_positions'],[])


if __name__=='__main__':
    unittest.main()
