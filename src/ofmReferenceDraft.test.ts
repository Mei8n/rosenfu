import {describe,it,expect} from 'vitest';
import {prepareOfmReferenceDraft as prepare,type DraftTileInput,type DraftTileLine} from './ofmReferenceDraft';
const seam=139.76806640625;
async function hash(data:Uint8Array){return Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',Uint8Array.from(data)))).map(n=>n.toString(16).padStart(2,'0')).join('');}
async function fixture(x:number):Promise<DraftTileInput>{
 const revision='rev',id=`ofm:rev:14:${x}:6451:1:0`,source=`https://tiles.openfreemap.org/planet/rev/14/${x}/6451.pbf`;
 const l:DraftTileLine={id,revision,z:14,x,y:6451,extent:4096,class:'minor',coordinates:[[seam-.0005,35.684],[seam+.0005,35.684]],oneway:0};
 const graph={dataset:{id:`ofm-tiles/rev/14/${x}/6451`,revision,source,license:'ODbL-1.0'},nodes:l.coordinates.map(([longitude,latitude],i)=>({osmId:id+':'+i,longitude,latitude})),edges:[{id:id+'/0',osmWayId:id,segmentIndex:0,from:id+':0',to:id+':1',mode:'road'}]};
 const rawTileBytes=new TextEncoder().encode(`synthetic-opaque-tile-${x}`);
 return {rawTileBytes,snapshot:{tileLines:[l],bundle:{graph,graphSha256:await hash(new TextEncoder().encode(JSON.stringify(graph))),sourceSha256:await hash(rawTileBytes),source:{source,sourceDate:null,license:'ODbL-1.0',attribution:'© OpenMapTiles © OpenStreetMap contributors'}}}};
}
const needed=[{x:14552,y:6451},{x:14553,y:6451}],start={fragmentId:'ofm:rev:14:14552:6451:1:0',segmentIndex:0,fraction:0},end={fragmentId:'ofm:rev:14:14553:6451:1:0',segmentIndex:0,fraction:1};
describe('isolated multi-tile draft',()=>{
 it('retains all context and source evidence without claiming an exact copy',async()=>{const a=await fixture(14552),b=await fixture(14553),before=JSON.stringify(a.snapshot);const r=await prepare([a,b],needed,start,end);expect(r.geometryDerived).toBe(true);expect(r.method).toBe('ofm_tile_path_v1');expect(r.newSources).toHaveLength(2);expect(r.newEvidence).toHaveLength(2);expect(r.newEvidence[0].tileLines[0].oneway).toBe(0);expect(r.path.orderedSpans.every(s=>r.newSources.some(t=>t.key===s.sourceKey))).toBe(true);expect(JSON.stringify(a.snapshot)).toBe(before);});
 it('references reusable evidence without duplicating it per alignment',async()=>{const inputs=[await fixture(14552),await fixture(14553)];const first=await prepare(inputs,needed,start,end);const reused=await prepare(inputs,needed,start,end,first.requiredSourceKeys);expect(reused.newEvidence).toEqual([]);expect(reused.newSources).toEqual([]);expect(reused.candidateContextKeys).toEqual(first.candidateContextKeys);expect(reused.path).toEqual(first.path);});
 it('rejects missing tiles',async()=>{await expect(prepare([await fixture(14552)],needed,start,end)).rejects.toThrow('tile-context-size');});
 it('rejects corrupted raw evidence',async()=>{const a=await fixture(14552);a.rawTileBytes[0]^=1;await expect(prepare([a,await fixture(14553)],needed,start,end)).rejects.toThrow('raw-hash');});
 it('rejects a changed graph hash',async()=>{const a=await fixture(14552);a.snapshot.bundle.graphSha256='0'.repeat(64);await expect(prepare([a,await fixture(14553)],needed,start,end)).rejects.toThrow('graph-hash');});
 it('rejects omitted competing context even with intact graph',async()=>{const a=await fixture(14552);a.snapshot.tileLines=[];await expect(prepare([a,await fixture(14553)],needed,start,end)).rejects.toThrow('incomplete-context');});
 it('rejects source URL and claimed revision mismatch',async()=>{const a=await fixture(14552);a.snapshot.bundle.source.source+='?x=1';await expect(prepare([a,await fixture(14553)],needed,start,end)).rejects.toThrow('source-url');});
 it('rejects excessive raw data before hashing',async()=>{const a=await fixture(14552);a.rawTileBytes=new Uint8Array(9*1024*1024);await expect(prepare([a,await fixture(14553)],needed,start,end)).rejects.toThrow('raw-budget');});
 it('rejects unknown raw one-way without silently assuming both directions',async()=>{const a=await fixture(14552),b=await fixture(14553);a.snapshot.tileLines=[{...a.snapshot.tileLines[0],oneway:'reversible'}];b.snapshot.tileLines=[{...b.snapshot.tileLines[0],oneway:'reversible'}];await expect(prepare([a,b],needed,start,end)).rejects.toThrow('disconnected');});
});
