import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync,writeFileSync,readFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {readWaterEvidencePack} from '../../src/waterEvidencePack.ts';

test('Python builder and TypeScript reader share Unicode, holes, line and unknown contracts',async()=>{
 const root=mkdtempSync(join(tmpdir(),'rosenfu-water-roundtrip-'));
 try{
  const id='🌊'.repeat(200),source=join(root,'source.geojson'),output=join(root,'pack');
  const ring=[[134.01,34.01],[134.09,34.01],[134.09,34.09],[134.01,34.09],[134.01,34.01]];
  const hole=[[134.03,34.03],[134.03,34.04],[134.04,34.04],[134.04,34.03],[134.03,34.03]];
  writeFileSync(source,JSON.stringify({type:'FeatureCollection',features:[
   {type:'Feature',id,properties:{evidence_kind:'water-area',source_tags:{name:'🌊'.repeat(3000)}},geometry:{type:'MultiPolygon',coordinates:[[ring,hole]]}},
   {type:'Feature',id:'stream/1',properties:{evidence_kind:'waterway-line'},geometry:{type:'LineString',coordinates:[[134.02,34.02],[134.08,34.08]]}}
  ]}));
  const builder=join(dirname(fileURLToPath(import.meta.url)),'build.py');
  const run=spawnSync(process.platform==='win32'?'python':'python3',[builder,source,'--bounds','134','34','134.1','34.1','--revision','fixture','--source-url','https://source.test/data','--license','test','--license-url','https://source.test/license','--attribution','🌊'.repeat(3000),'--output',output],{encoding:'utf8'});
  assert.equal(run.status,0,run.stderr);const report=JSON.parse(run.stdout);
  const manifest=readFileSync(join(output,'manifest.json')),payload=readFileSync(join(output,'evidence.json'));
  assert.equal(createHash('sha256').update(manifest).digest('hex'),report.manifest_sha256);
  const pack=await readWaterEvidencePack(manifest,report.manifest_sha256,payload);
  assert.equal(pack.status,'incomplete');assert.deepEqual(pack.coverage,[]);
  assert.equal(pack.water.length,1);assert.equal(pack.water[0].rings.length,2);
  assert.ok(pack.observations.some(o=>o.id===id));assert.equal(pack.observations.length,2);
 }finally{rmSync(root,{recursive:true,force:true});}
});
