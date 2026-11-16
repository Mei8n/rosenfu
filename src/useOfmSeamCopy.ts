import {useEffect,useRef,useState} from 'react';
import {prepareSharedOfmEvidenceDraft,type TileEvidenceBinding} from './ofmSharedEvidenceDraft';
import type {DraftTileSnapshot} from './ofmReferenceDraft';
import type {TilePathPick} from './ofmDerivedTilePath';
export type SeamEvidenceRequest={revision:string;z:14;x:number;y:number;rawSha256:string;decoderRule:'ofm_transportation_z14_v1'};
export type SeamEvidenceBatch={rawTiles:{sha256:string;bytes:Uint8Array}[];bindings:TileEvidenceBinding[]};
export type SeamEvidenceReader=(refs:readonly SeamEvidenceRequest[],signal?:AbortSignal)=>Promise<{status:'ready';evidence:SeamEvidenceBatch}|{status:'unavailable';reason:string}|{status:'stale'}>;
export type SeamDraft=Awaited<ReturnType<typeof prepareSharedOfmEvidenceDraft>>;
export type SeamCopyOptions={enabled:boolean;world:unknown;tool:string;frozenTiles:readonly DraftTileSnapshot[];requiredTiles:readonly {x:number;y:number}[];start:TilePathPick|null;end:TilePathPick|null;readTileEvidenceBatch:SeamEvidenceReader};
export type SeamCopyState={status:'idle'}|{status:'loading'}|{status:'ready';draft:SeamDraft}|{status:'error';message:string};
/** Isolated preview preparation only: no App, backend command, construction or save writes.
 * Supply the SAME repository reader that observed frozenTiles, never a new repository. */
export function useOfmSeamCopy(options:SeamCopyOptions){
 const sequence=useRef(0),active=useRef<AbortController|null>(null);
 const identity=JSON.stringify([options.tool,options.enabled,options.requiredTiles,options.start,options.end]);
 const current=useRef({options,identity,epoch:0});
 if(current.current.identity!==identity||current.current.options.world!==options.world||current.current.options.frozenTiles!==options.frozenTiles||current.current.options.readTileEvidenceBatch!==options.readTileEvidenceBatch){current.current={options,identity,epoch:current.current.epoch+1};}else current.current.options=options;
 const epoch=current.current.epoch;
 const [result,setResult]=useState<{epoch:number;state:SeamCopyState}>({epoch,state:{status:'idle'}});
 useEffect(()=>{active.current?.abort();sequence.current++;setResult({epoch,state:{status:'idle'}});return()=>{active.current?.abort();sequence.current++;};},[epoch]);
 function cancel(){sequence.current++;active.current?.abort();active.current=null;setResult({epoch:current.current.epoch,state:{status:'idle'}});}
 async function prepare():Promise<SeamDraft|null>{
  const captured=current.current,o=captured.options;
  if(!o.enabled||o.tool!=='tracks'||!o.start||!o.end)return null;
  active.current?.abort();const controller=new AbortController();active.current=controller;const token=++sequence.current;
  const valid=()=>!controller.signal.aborted&&sequence.current===token&&current.current.epoch===captured.epoch;
  setResult({epoch:captured.epoch,state:{status:'loading'}});
  try{
   // Caller-owned snapshots/picks cannot change the in-flight evidence request.
   const json=JSON.stringify({tiles:o.frozenTiles,required:o.requiredTiles,start:o.start,end:o.end});
   if(new TextEncoder().encode(json).length>16*1024*1024)throw Error('snapshot-budget');
   const frozen=JSON.parse(json) as {tiles:DraftTileSnapshot[];required:{x:number;y:number}[];start:TilePathPick;end:TilePathPick};
   if(!frozen.tiles.length||frozen.tiles.length>16||frozen.tiles.length!==frozen.required.length)throw Error('tile-context-size');
   const needed=new Set(frozen.required.map(t=>`${t.x}/${t.y}`));
   if(needed.size!==frozen.required.length)throw Error('required-tiles');
   const requests=frozen.tiles.map(t=>{
    const m=t.bundle.source.source.match(/^https:\/\/tiles\.openfreemap\.org\/planet\/([A-Za-z0-9_]{1,64})\/14\/(0|[1-9]\d{0,4})\/(0|[1-9]\d{0,4})\.pbf$/);
    if(!m)throw Error('source-url');const x=Number(m[2]),y=Number(m[3]);if(x>=16384||y>=16384||!needed.delete(`${x}/${y}`))throw Error('required-tiles');
    return {revision:m[1],z:14 as const,x,y,rawSha256:t.bundle.sourceSha256,decoderRule:'ofm_transportation_z14_v1' as const};
   });
   if(needed.size)throw Error('missing-tile');
   const response=await o.readTileEvidenceBatch(requests,controller.signal);if(!valid())return null;
   if(response.status==='stale'){setResult({epoch:captured.epoch,state:{status:'idle'}});return null;}
   if(response.status==='unavailable')throw Error(response.reason);
   const batch=response.evidence;
   if(batch.bindings.length!==requests.length||batch.rawTiles.length>16)throw Error('evidence-response');
   const blobs=new Map<string,Uint8Array>();let total=0;
   for(const raw of batch.rawTiles){total+=raw.bytes.length;if(total>8*1024*1024||blobs.has(raw.sha256))throw Error('evidence-response');blobs.set(raw.sha256,raw.bytes.slice());}
   const bindings=new Map(batch.bindings.map(b=>[b.key,b]));if(bindings.size!==requests.length)throw Error('evidence-response');
   const used=new Set<string>();
   const inputs=requests.map((r,i)=>{const snapshot=frozen.tiles[i],url=snapshot.bundle.source.source,b=bindings.get(url),raw=blobs.get(r.rawSha256);
    if(!b||!raw||b.url!==url||b.revision!==r.revision||b.x!==r.x||b.y!==r.y||b.z!==14||b.rawSha256!==r.rawSha256||b.decoderRule!==r.decoderRule||b.license!==snapshot.bundle.source.license||b.attribution!==snapshot.bundle.source.attribution)throw Error('evidence-binding');
    used.add(r.rawSha256);return {snapshot,rawTileBytes:raw};});
   if(used.size!==blobs.size)throw Error('unexpected-evidence');
   const draft=await prepareSharedOfmEvidenceDraft(inputs,frozen.required,frozen.start,frozen.end);
   if(!valid())return null;setResult({epoch:captured.epoch,state:{status:'ready',draft}});return draft;
  }catch(error){if(!valid())return null;setResult({epoch:captured.epoch,state:{status:'error',message:String(error)}});return null;}
  finally{if(active.current===controller)active.current=null;}
 }
 return {state:result.epoch===epoch?result.state:{status:'idle'} as SeamCopyState,prepare,cancel};
}
