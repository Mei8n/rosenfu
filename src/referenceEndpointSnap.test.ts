import {expect,test} from 'vitest';
import {syntheticReferenceGraph} from './fixtures/referenceGraph';
import {snapReferenceStart} from './referenceEndpointSnap';
test('source-preserving snap recomputes fractional start but rejects parallel and out-of-segment points',()=>{
 const graph=syntheticReferenceGraph();const edge=graph.edges[0],a=graph.nodes.find(n=>n.osmId===edge.from)!,b=graph.nodes.find(n=>n.osmId===edge.to)!;
 const bundle={graph,graphSha256:'a'.repeat(64),sourceSha256:'b'.repeat(64),source:{source:'synthetic',sourceDate:null,license:'ODbL-1.0',attribution:'OSM'}};
 const pick={datasetId:graph.dataset.id,revision:graph.dataset.revision,edgeId:edge.id,fraction:.4};
 const anchor={longitude:a.longitude+(b.longitude-a.longitude)*.25,latitude:a.latitude+(b.latitude-a.latitude)*.25};
 const result=snapReferenceStart(bundle,[pick,{...pick,fraction:1}],anchor,'reference');
 expect(result?.points[0]).toEqual(anchor);expect(result?.provenance.start_fraction).toBeCloseTo(.25);
 expect(snapReferenceStart(bundle,[pick,{...pick,fraction:1}],{...anchor,latitude:anchor.latitude+.000001},'reference')).toBeNull();
 expect(snapReferenceStart(bundle,[pick,{...pick,fraction:1}],{longitude:a.longitude-(b.longitude-a.longitude),latitude:a.latitude-(b.latitude-a.latitude)},'reference')).toBeNull();
});
