import {prepareOfmReferenceDraft,type DraftTileInput} from './ofmReferenceDraft';
import type {TilePathPick} from './ofmDerivedTilePath';

export const OFM_RAW_INPUT_LIMIT=8*1024*1024;
export const OFM_WORLD_JSON_LIMIT=16*1024*1024;
export type RawTileEvidence={sha256:string;byteLength:number;base64:string};
export type TileEvidenceBinding={key:string;rawSha256:string;url:string;revision:string;z:14;x:number;y:number;decoderRule:'ofm_transportation_z14_v1';license:string;attribution:string};
/** Strict canonical encoding, including zero padding bits; bounds checked before allocation. */
export function decodeCanonicalTileEvidence(e:RawTileEvidence):Uint8Array{
 if(!/^[a-f0-9]{64}$/.test(e.sha256)||!Number.isSafeInteger(e.byteLength)||e.byteLength<1||e.byteLength>5*1024*1024)throw Error('raw-evidence-size');
 if(e.base64.length!==4*Math.ceil(e.byteLength/3)||! /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(e.base64))throw Error('raw-base64');
 const binary=atob(e.base64);if(binary.length!==e.byteLength||btoa(binary)!==e.base64)throw Error('raw-base64');
 return Uint8Array.from(binary,c=>c.charCodeAt(0));
}
/** Caller must pass the entire proposed world, including existing source table and all other data.
 * This is a sizing check only, never persistence or a substitute for backend validation. */
export function assertProposedWorldBudget(proposedWorld:unknown):number{
 const size=new TextEncoder().encode(JSON.stringify(proposedWorld)).length;
 if(size>OFM_WORLD_JSON_LIMIT)throw Error('world-json-budget');return size;
}
export async function prepareSharedOfmEvidenceDraft(inputs:readonly DraftTileInput[],requiredTiles:readonly {x:number;y:number}[],start:TilePathPick,end:TilePathPick,existingRawHashes:readonly string[]=[]){
 if(existingRawHashes.length>1024||existingRawHashes.some(h=>!/^[a-f0-9]{64}$/.test(h)))throw Error('reuse-hashes');
 // Existing values are hints only. Backend must resolve, rehash and decode each required source.
 const existing=new Set(existingRawHashes);
 const draft=await prepareOfmReferenceDraft(inputs,requiredTiles,start,end);
 const raw=new Map<string,RawTileEvidence>();const bindings:TileEvidenceBinding[]=[];
 const keyBySource=new Map<string,string>();
 for(const source of draft.newSources){
  const evidence=draft.newEvidence.find(e=>e.sourceKey===source.key)!;
  const value:RawTileEvidence={sha256:source.source_sha256,byteLength:evidence.rawByteLength,base64:evidence.rawTileBase64};
  decodeCanonicalTileEvidence(value);
  const previous=raw.get(value.sha256);
  if(previous&&(previous.base64!==value.base64||previous.byteLength!==value.byteLength))throw Error('raw-evidence-conflict');
  raw.set(value.sha256,value);
  // URL binds the same raw bytes independently to each coordinate/revision; never infer aliasing.
  const key=source.source;keyBySource.set(source.key,key);
  bindings.push({key,rawSha256:value.sha256,url:source.source,revision:source.revision,z:14,x:evidence.x,y:evidence.y,decoderRule:draft.decoderRule,license:source.license,attribution:source.attribution});
 }
 const alignment={method:draft.method,algorithm:'ofm_overlap_blend_v1' as const,geometryDerived:true as const,coverageVerified:false as const,requiredBindingKeys:bindings.map(b=>b.key),start:draft.start,end:draft.end,
  orderedSpans:draft.path.orderedSpans.map(s=>({bindingKey:keyBySource.get(s.sourceKey)!,fragmentId:s.fragmentId,from:s.from,to:s.to})),
  joins:draft.path.joins.map(j=>({rule:j.rule,aId:j.aId,bId:j.bId,axis:j.axis,fromAxis:j.fromAxis,toAxis:j.toAxis,aRange:j.aRange,bRange:j.bRange}))};
 const persistentProposal={version:2 as const,requiredRawHashes:[...raw.keys()],newRawEvidence:[...raw.values()].filter(e=>!existing.has(e.sha256)),tileBindings:bindings,alignment};
 // Explicitly ephemeral: no raw/base64 duplication, no repeated full selected-span geometry.
 const temporaryValidationContext=draft.newEvidence.map(e=>({bindingKey:keyBySource.get(e.sourceKey)!,graphJson:e.graphJson,tileLines:e.tileLines}));
 const result={format:'rosenfu-ofm-shared-evidence-draft' as const,version:2 as const,persistentProposal,temporaryValidationContext};
 if(new TextEncoder().encode(JSON.stringify(result)).length>OFM_WORLD_JSON_LIMIT)throw Error('ipc-json-budget');
 return result;
}
