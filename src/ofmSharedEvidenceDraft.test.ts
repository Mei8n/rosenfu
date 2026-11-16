import {describe,it,expect} from 'vitest';
import type {DraftTileInput,DraftTileLine} from './ofmReferenceDraft';
import {prepareSharedOfmEvidenceDraft as prepare,decodeCanonicalTileEvidence,assertProposedWorldBudget,OFM_WORLD_JSON_LIMIT} from './ofmSharedEvidenceDraft';
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
describe('shared evidence proposal',()=>{
 it('separates reusable raw evidence and light descriptors from ephemeral decoded context',async()=>{const input=[await fixture(14552),await fixture(14553)];const result=await prepare(input,needed,start,end);const p=result.persistentProposal;expect(p.newRawEvidence).toHaveLength(2);expect(result.temporaryValidationContext).toHaveLength(2);const json=JSON.stringify(p);expect(json).not.toContain('rawCoordinates');expect(json).not.toContain('graphJson');expect(json).not.toContain('tileLines');const again=await prepare(input,needed,start,end,p.requiredRawHashes);expect(again.persistentProposal.newRawEvidence).toEqual([]);expect(again.persistentProposal.alignment).toEqual(p.alignment);});
 it('deduplicates identical raw bytes but retains distinct tile bindings',async()=>{const a=await fixture(14552),b=await fixture(14553);b.rawTileBytes=a.rawTileBytes.slice();b.snapshot.bundle.sourceSha256=a.snapshot.bundle.sourceSha256;const p=(await prepare([a,b],needed,start,end)).persistentProposal;expect(p.newRawEvidence).toHaveLength(1);expect(p.tileBindings).toHaveLength(2);expect(p.tileBindings[0].key).not.toBe(p.tileBindings[1].key);expect(p.tileBindings[0].rawSha256).toBe(p.tileBindings[1].rawSha256);});
 it('rejects noncanonical padding bits, whitespace and length mismatches',()=>{const e={sha256:'a'.repeat(64),byteLength:1,base64:'AA=='};expect(decodeCanonicalTileEvidence(e)).toEqual(new Uint8Array([0]));for(const base64 of ['AB==','AA==\n','AA','AAA='])expect(()=>decodeCanonicalTileEvidence({...e,base64})).toThrow('raw-base64');});
 it('checks complete proposed world at the exact 16MiB JSON boundary',()=>{expect(assertProposedWorldBudget('a'.repeat(OFM_WORLD_JSON_LIMIT-2))).toBe(OFM_WORLD_JSON_LIMIT);expect(()=>assertProposedWorldBudget('a'.repeat(OFM_WORLD_JSON_LIMIT-1))).toThrow('world-json-budget');});
 it('retains the 8MiB raw input guard even for requested reuse',async()=>{const a=await fixture(14552);a.rawTileBytes=new Uint8Array(8*1024*1024+1);await expect(prepare([a,await fixture(14553)],needed,start,end,[a.snapshot.bundle.sourceSha256])).rejects.toThrow('raw-budget');});
});
