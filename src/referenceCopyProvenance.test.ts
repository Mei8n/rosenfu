import {expect,test} from 'vitest';
import {prepareReferenceCopy} from './referenceCopy';
import {syntheticReferenceGraph} from './fixtures/referenceGraph';
import type {ReferenceGraphBundle} from './referenceGraphBundle';
function fixture(tile=false):ReferenceGraphBundle {
 const graph=syntheticReferenceGraph();
 graph.nodes=graph.nodes.slice(0,2);graph.edges=graph.edges.slice(0,1);
 if(tile){
  const revision='20261101_080013_pt',fragment=`ofm:${revision}:14:14551:6465:12:0`;
  graph.dataset.id=`ofm-tiles/${revision}`;graph.dataset.revision=revision;graph.dataset.source=`https://tiles.openfreemap.org/planet/${revision}`;
  graph.nodes.forEach((n,i)=>n.osmId=`${fragment}:${i}`);
  Object.assign(graph.edges[0],{id:`${fragment}:0`,osmWayId:fragment,from:graph.nodes[0].osmId,to:graph.nodes[1].osmId,mode:'road'});
 }
 return {graph,graphSha256:'a'.repeat(64),sourceSha256:'b'.repeat(64),source:{source:'synthetic reference',sourceDate:null,license:'ODbL-1.0',attribution:'© OpenStreetMap contributors'}};
}
function copy(bundle:ReferenceGraphBundle){const pick={datasetId:bundle.graph.dataset.id,revision:bundle.graph.dataset.revision,edgeId:bundle.graph.edges[0].id};return prepareReferenceCopy(bundle,{...pick,fraction:0},{...pick,fraction:1},'Reference');}
test('OSM copy preserves strict historical provenance with no tile identity',()=>{
 const result=copy(fixture());expect(result.provenance.method).toBe('osm_way_slice_v1');expect(result.provenance.osm_way_id).toMatch(/^[1-9]\d*$/);expect(result.provenance.tile_fragment_id).toBeUndefined();expect(result.sourceRecord.conversion_format_version).toBe(1);
});
test('tile copy persists tile identity separately and never invents an OSM way ID',()=>{
 const bundle=fixture(true),result=copy(bundle);expect(result.provenance).toMatchObject({method:'ofm_tile_slice_v1',osm_way_id:'',tile_fragment_id:bundle.graph.edges[0].osmWayId,mode:'road'});expect(result.sourceRecord.conversion_format_version).toBe(3);expect(result.points).toEqual(bundle.graph.nodes.map(({latitude,longitude})=>({latitude,longitude})));
});
test('tile identity cannot pass as an ordinary OSM dataset',()=>{const bundle=fixture(true);bundle.graph.dataset.id='ordinary';expect(()=>copy(bundle)).toThrow();});
