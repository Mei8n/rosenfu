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

test('grid catalog leaves and clipped marine polygons preserve the reader contract',async()=>{
 const root=mkdtempSync(join(tmpdir(),'rosenfu-water-grid-marine-'));
 try{
  const scripts=dirname(fileURLToPath(import.meta.url));
  const script=`import sys,json
from pathlib import Path
sys.path.insert(0,sys.argv[1])
from grid import build_grid
from marine import build_marine
from test_marine import MarineTests
root=Path(sys.argv[2]); source=root/'source.json'
source.write_text(json.dumps(dict(type='FeatureCollection',features=[])))
build_grid(source,[[134,34,134.1,34.1],[135,34,135.1,34.1]],'fixture','https://source.test','test','https://license.test','test',root/'grid')
MarineTests().fixture(root/'sea.shp')
print(json.dumps(build_marine(root/'sea.shp',[134.02,34.02,134.1,34.1],'fixture','https://source.test',root/'marine')))
`;
  const run=spawnSync(process.platform==='win32'?'python':'python3',['-c',script,scripts,root],{encoding:'utf8'});
  assert.equal(run.status,0,run.stderr);
  const catalog=JSON.parse(readFileSync(join(root,'grid/catalog.json')));
  assert.deepEqual(catalog.coverage,[]);
  for(const entry of catalog.cells){
   const directory=join(root,'grid',entry.directory);
   const pack=await readWaterEvidencePack(readFileSync(join(directory,'manifest.json')),entry.manifest_sha256,readFileSync(join(directory,'evidence.json')));
   assert.equal(pack.status,'incomplete');assert.deepEqual(pack.coverage,[]);assert.equal(pack.water.length,0);
  }
  const report=JSON.parse(run.stdout),directory=join(root,'marine');
  const pack=await readWaterEvidencePack(readFileSync(join(directory,'manifest.json')),report.manifest_sha256,readFileSync(join(directory,'evidence.json')));
  assert.equal(pack.status,'incomplete');assert.deepEqual(pack.coverage,[]);
  assert.equal(pack.water.length,1);assert.equal(pack.water[0].rings.length,2);
 }finally{rmSync(root,{recursive:true,force:true});}
});

test('sealed disk stage extracts and composes without claiming common source time',async()=>{
 const root=mkdtempSync(join(tmpdir(),'rosenfu-water-stage-composite-'));
 try{
  const scripts=dirname(fileURLToPath(import.meta.url));
  const script=`import sys,json,hashlib
from pathlib import Path
sys.path.insert(0,sys.argv[1])
from test_source_set import SourceSetTests
from source_set import audit_sources
from build import encoded,build
from stage import stage_region,extract_stage
from compose import compose
root=Path(sys.argv[2]);receipts=SourceSetTests().fixture(root)
raw=encoded(audit_sources(receipts,'2026-11-14T21:20:41Z'));(root/'sources.json').write_bytes(raw)
report=stage_region(root/'kanto-261114.osm.pbf',root/'sources.json',hashlib.sha256(raw).hexdigest(),'kanto',root/'stage')
a=extract_stage(root/'stage',report['stage_manifest_sha256'],[134,34,134.1,34.1],root/'inland')
(root/'source.json').write_bytes(encoded(dict(type='FeatureCollection',features=[])))
b=build(root/'source.json',[134,34,134.1,34.1],'different-source-date','https://source.test','ODbL-1.0','https://license.test','OSM',root/'marine')
print(json.dumps(compose(root/'inland',a['manifest_sha256'],root/'marine',b['manifest_sha256'],root/'composite')))
`;
  const run=spawnSync(process.platform==='win32'?'python':'python3',['-B','-c',script,scripts,root],{encoding:'utf8'});assert.equal(run.status,0,run.stderr);
  const {readWaterEvidenceComposite}=await import('../../src/waterEvidenceComposite.ts');
  const files=new Map();for(const key of ['inland','marine'])for(const name of ['manifest.json','evidence.json'])files.set(`${key}/${name}`,readFileSync(join(root,'composite',key,name)));
  const pack=await readWaterEvidenceComposite(readFileSync(join(root,'composite/composite.json')),JSON.parse(run.stdout).composite_sha256,files);
  assert.equal(pack.status,'incomplete');assert.deepEqual(pack.coverage,[]);assert.equal(pack.hasMixedRevisionClaims,true);
 }finally{rmSync(root,{recursive:true,force:true});}
});
