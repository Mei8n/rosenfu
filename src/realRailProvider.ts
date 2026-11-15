import {createRailNetwork,decodeRailGraph,type RailPick,type RailRoute} from './realRailGraph';
import type {GraphBounds,GraphPoint} from './referenceGraph';
import type {SourceRecord} from './AlignmentProvenance';
// Verified against the published tw-data meta and repository data-version.txt.
// Never fetch city/statistical/boundary packs. See docs/automatic-real-rail.md.
export const RAIL_RELEASE='2026-11-13';
export const RAIL_BASE=`https://data.anita.garden/trainworld/${RAIL_RELEASE}/realgraph/`;
export const RAIL_SOURCE=RAIL_BASE+'jp.rgr';
export type RailView={chains:{id:number;points:GraphPoint[]}[];truncated:boolean};
export type RailRequest={id:number}&({kind:'load'}|{kind:'view';bounds:GraphBounds}|{kind:'route';start:RailPick;end:RailPick}|{kind:'pick';point:GraphPoint;radius:number;visibleIds:number[];start:RailPick|null});
export type RailResponse={id:number}&({kind:'loaded';source:SourceRecord}|{kind:'view';view:RailView}|{kind:'pick';pick:RailPick|null;route:RailRoute|null}|{kind:'error';message:string});
async function limited(stream:ReadableStream<Uint8Array>,limit:number,signal:AbortSignal){
 const reader=stream.getReader();const parts:Uint8Array[]=[];let size=0;
 try{while(true){if(signal.aborted)throw Error('取得をキャンセルしました');const {done,value}=await reader.read();if(done)break;size+=value.byteLength;if(size>limit)throw Error('取得サイズ上限です');parts.push(value);}}
 catch(error){await reader.cancel();throw error;}finally{reader.releaseLock();}
 const bytes=new Uint8Array(size);let offset=0;for(const p of parts){bytes.set(p,offset);offset+=p.length;}return bytes;
}
async function download(url:string,limit:number,signal:AbortSignal){const response=await fetch(url,{signal,credentials:'omit',referrerPolicy:'no-referrer'});if(!response.ok||!response.body)throw Error(`線路取得失敗 HTTP ${response.status}`);return limited(response.body,limit,signal);}
async function digest(bytes:Uint8Array){return [...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes as Uint8Array<ArrayBuffer>))].map(n=>n.toString(16).padStart(2,'0')).join('');}
export async function loadRailProvider(signal:AbortSignal){
 const index=JSON.parse(new TextDecoder().decode(await download(RAIL_BASE+'index.json',16384,signal)));
 if(!Array.isArray(index.regions)||!index.regions.includes('jp'))throw Error('この版の日本の線路coverageは未提供です');
 const compressed=await download(RAIL_SOURCE,8*1024*1024,signal);
 const stream=new Blob([compressed as Uint8Array<ArrayBuffer>]).stream().pipeThrough(new DecompressionStream('gzip'));
 const raw=await limited(stream,32*1024*1024,signal);
 const graph=decodeRailGraph(raw.buffer),graphHash=await digest(raw);
 const metadata={provider:'TrainsWorld/noritetsu OSM rail-only graph',release:RAIL_RELEASE,region:'jp',format:'TWRG/1',url:RAIL_SOURCE,license:'ODbL-1.0',attribution:'© OpenStreetMap contributors'};
 const sourceHash=await digest(new TextEncoder().encode(JSON.stringify(metadata)));
 const source:SourceRecord={key:graphHash+':'+sourceHash,dataset_id:'trainsworld-realgraph-jp',revision:RAIL_RELEASE,graph_sha256:graphHash,source_sha256:sourceHash,source:RAIL_SOURCE,source_date:null,license:'ODbL-1.0',attribution:'© OpenStreetMap contributors',conversion_format_version:2};
 return {network:createRailNetwork(graph),source};
}
