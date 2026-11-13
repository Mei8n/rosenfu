import { expect, test } from 'vitest';
import { createReferenceSpatialIndex, boundsOverlap } from './referenceSpatialIndex';
import { eligibleRoadGraph, validateRoadPicks } from './roadFollowing';
import { syntheticReferenceGraph } from './fixtures/referenceGraph';
import type { ReferenceGraphBundle } from './referenceGraphBundle';

test('recovered spatial index preserves scan ordering and immutable bounds', () => {
 const bounds=Array.from({length:250},(_,i)=>({west:i%25,south:Math.floor(i/25),east:i%25+2,north:Math.floor(i/25)+2}));
 const index=createReferenceSpatialIndex(bounds);
 for(let i=0;i<20;i++) {
  const query={west:i,south:2,east:i+3,north:5};
  expect(index.query(query)).toEqual(bounds.flatMap((b,n)=>boundsOverlap(b,query)?[n]:[]));
 }
 const query={west:0,south:0,east:1,north:1};const before=index.query(query);
 bounds[0].west=1000;expect(index.query(query)).toEqual(before);
 expect(createReferenceSpatialIndex([]).query(query)).toEqual([]);
});
function bundle():ReferenceGraphBundle {
 const graph=syntheticReferenceGraph();
 return {graph,graphSha256:'0'.repeat(64),sourceSha256:'1'.repeat(64),source:{source:'synthetic',sourceDate:null,license:'test',attribution:'test'},roadWays:{'105':{highway:'residential',access:null,motorVehicle:null,structureUnsupported:false}}};
}
test('recovered tram follow fails closed without metadata and excludes unsafe roads',()=>{
 const b=bundle();expect(eligibleRoadGraph(b).edges.map(e=>e.id)).toEqual(['105']);
 expect(eligibleRoadGraph({...b,roadWays:undefined}).edges).toEqual([]);
 for(const patch of [{highway:'motorway'},{highway:'primary_link'},{access:'private'},{motorVehicle:'no'},{structureUnsupported:true}]) {
  expect(eligibleRoadGraph({...b,roadWays:{'105':{...b.roadWays!['105'],...patch}}}).edges).toEqual([]);
 }
 for(const patch of [{layer:1},{level:'1'},{bridge:true},{tunnel:true}]) {
  expect(eligibleRoadGraph({...b,graph:{...b.graph,edges:b.graph.edges.map(e=>e.id==='105'?{...e,...patch}:e)}}).edges).toEqual([]);
 }
});
test('recovered road selection rejects stale identity, unsupported edges and invalid fractions',()=>{
 const b=bundle();const valid={datasetId:b.graph.dataset.id,revision:b.graph.dataset.revision,edgeId:'105',fraction:.5};
 expect(()=>validateRoadPicks(b,[valid])).not.toThrow();
 for(const patch of [{revision:'stale'},{datasetId:'other'},{edgeId:'104'},{fraction:NaN},{fraction:2}]) expect(()=>validateRoadPicks(b,[{...valid,...patch}])).toThrow();
 expect(()=>validateRoadPicks(b,[])).toThrow();
});
