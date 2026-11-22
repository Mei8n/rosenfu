import base64
import hashlib
import json
import re
from pathlib import Path
import shutil
import subprocess
import unittest

from fixture import CAMERA,LEFT,RIGHT,METADATA,config,projection
from smoke import FETCH_FIXTURE
from smoke_authored_tram import START,JOINT,END,STATIONS,road_input,distance_m,assert_authored_shape
from wd_native import nav_click_payload


class Fixture(unittest.TestCase):
    def test_strict_canonical_response_list_and_hashes(self):
        for scenario,count in [('single',1),('seam',2),('loop',1)]:
            c=config(scenario)
            self.assertEqual(c['meta'],METADATA)
            self.assertTrue(all(re.match(r'^https://tiles\.openfreemap\.org/planet/rosenfu_webview_fixture_16/14/\d+/\d+\.pbf$',u) for u in c['responses']))
            self.assertEqual(len(c['primaryShas']),count)
            for name,digest in c['primaryShas'].items():
                self.assertEqual(hashlib.sha256(base64.b64decode(c['responses'][name])).hexdigest(),digest)
            self.assertEqual(set(a['key'] for a in c['anchors']),set(c['primaryShas']))

    def test_east_to_west_map_clicks_visible_and_separated(self):
        self.assertEqual(projection(CAMERA[:2]),[500,300])
        for scenario in ['single','seam']:
            start,end=[projection(a['geo']) for a in config(scenario)['anchors']]
            self.assertGreater(start[0],end[0]+40)
            for x,y in (start,end):
                self.assertTrue(250<x<720 and 300<y<450,(x,y))
                events=nav_click_payload(x,y)['actions'][0]['actions']
                self.assertEqual(events[0]['origin'],'viewport')
                self.assertEqual([a['type'] for a in events],['pointerMove','pointerDown','pointerUp'])

    def test_circuit_is_three_distinct_interior_controls(self):
        anchors=config('loop')['anchors']
        self.assertEqual([(a['feature'],a['vertex']) for a in anchors],[(0,0),(0,1),(1,1)])
        self.assertEqual(len({a['geo'] for a in anchors}),3)
        for a in anchors:
            x,y=projection(a['geo'])
            self.assertTrue(410<x<560 and 350<y<485,(x,y))

    def test_authored_tram_fixture_t_junction_and_ordinary_persistence_contract(self):
        c,target=road_input()
        self.assertIn("/authored_tram_webview_16/14/14552/6452.pbf",target)
        self.assertEqual(hashlib.sha256(base64.b64decode(c["responses"][target])).hexdigest(),"4582f5d0dddebd0695d857efda04e75197dddf4b54ab78700a018f0233d5a959")
        for a in (START,JOINT,END,*STATIONS):
            x,y=projection(a)
            self.assertTrue(380<x<550 and 305<y<465)
        self.assertGreater(distance_m(START,JOINT),30)
        self.assertGreater(distance_m(JOINT,END),30)
        self.assertGreater(distance_m(START,STATIONS[0]),40)
        self.assertGreater(distance_m(END,STATIONS[1]),40)
        self.assertGreater(distance_m(STATIONS[0],STATIONS[1]),120)
        self.assertGreater(distance_m(((START[0]+END[0])/2,(START[1]+END[1])/2),JOINT),20)
        points=lambda positions:[{"latitude":lat,"longitude":lon} for lat,lon in positions]
        arc={"track_type":"tram","points":points([START,JOINT,JOINT,END])}
        self.assertEqual(assert_authored_shape({"alignments":[arc]}),arc)
        chord={"track_type":"tram","points":points([START,START,END,END])}
        with self.assertRaisesRegex(AssertionError,"omitted"):
            assert_authored_shape({"alignments":[chord]})
        with self.assertRaisesRegex(AssertionError,"operational road authority"):
            assert_authored_shape({"alignments":[arc],"road_transport":{"tram_sections":[{}]}})

    @unittest.skipUnless(shutil.which('node'),"Node needed for network shim contract")
    def test_only_exact_provider_urls_intercepted_no_ipc_change(self):
        code=r"""
const assert=require('node:assert/strict');
const {webcrypto}=require('node:crypto');
globalThis.crypto=webcrypto;
const [impl,c]=JSON.parse(require('node:fs').readFileSync(0,'utf8'));
const real=[];
const fetch=async (...x)=>{real.push(x);return new Response('untouched');};
const invoke=()=>{throw Error('network shim must never call IPC')};
globalThis.window={fetch,__TAURI_INTERNALS__:{invoke}};
(async ()=>{
 assert(new Function(impl)(c));
 const tile=Object.keys(c.primaryShas)[0];
 assert.equal((await (await window.fetch(c.meta)).json()).tiles[0],c.template);
 const body=Buffer.from(await (await window.fetch(new Request(tile))).arrayBuffer());
 assert.equal(body.toString('base64'),c.responses[tile]);
 const miss=['http://ipc.localhost/preview_gameplay_ofm_bus',tile+'?other',c.meta+'/other',
   'https://tiles.openfreemap.org.evil.test/planet','https://example.test/foo'];
 for(const k of miss) assert.equal(await (await window.fetch(k,{credentials:'omit'})).text(),'untouched');
 assert.deepEqual(real.map(a=>a[0]),miss);
 assert.deepEqual(window.__ordinaryBusNetworkSmoke,[c.meta,tile]);
 assert.equal(window.__TAURI_INTERNALS__.invoke,invoke);
})().catch(e=>{console.error(e);process.exitCode=1});
"""
        p=subprocess.run(['node','-e',code],input=json.dumps([FETCH_FIXTURE,config('seam')]),capture_output=True,text=True)
        self.assertEqual(p.returncode,0,p.stderr)


if __name__=='__main__':
    unittest.main()
