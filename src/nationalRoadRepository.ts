import {createRoadPackRepository} from './roadPackRepository';
import {roadBoundsKey,type RoadSnapshot} from './automaticRoadData';
import type {GraphBounds} from './referenceGraph';
type Shard={bounds:number[];manifest:string;sha256:string;sourceUrl:string;sourceSha:string;revision:string;coverageVerified:boolean};
const HASH=/^[a-f0-9]{64}$/;
const MAX_INDEX_BYTES=5*1024*1024;
function object(value:unknown):Record<string,unknown>{if(!value||typeof value!=='object'||Array.isArray(value))throw Error('全国道路index形式不正');return value as Record<string,unknown>;}
export function parseNationalRoadIndex(value:unknown):Shard[]{
 const root=object(value);
 if(root.format!=='rosenfu-road-pack-index'||root.version!==1||typeof root.coverage_verified!=='boolean'||(root.coverage_verified?root.polygon_snapshot_match!=='verified':root.polygon_snapshot_match!=='unverified')||root.license!=='ODbL-1.0'||typeof root.attribution!=='string'||!root.attribution||typeof root.revision!=='string'||!Number.isFinite(Date.parse(root.revision))||!Array.isArray(root.regions)||root.regions.length>16)throw Error('全国道路index契約不一致');
 const result:Shard[]=[],seen=new Set<string>();
 for(const item of root.regions){const region=object(item),source=object(region.source);
  if(typeof region.name!=='string'||!/^[A-Za-z0-9-]+$/.test(region.name)||typeof source.url!=='string'||!source.url.startsWith('https://download.geofabrik.de/')||typeof source.sha256!=='string'||!HASH.test(source.sha256)||source.timestamp!==root.revision||!Array.isArray(region.shards))throw Error('全国道路source不一致');
  for(const entry of region.shards){const shard=object(entry),b=shard.bounds,path=shard.manifest;
   if(!Array.isArray(b)||b.length!==4||!b.every(n=>Number.isFinite(n)&&Number.isInteger(n))||b[0]<-180||b[2]>180||b[1]<-85||b[3]>85||b[2]!==b[0]+1||b[3]!==b[1]+1||path!==`packs/${region.name}/${b[0]}_${b[1]}/manifest.json`||typeof shard.sha256!=='string'||!HASH.test(shard.sha256)||seen.has(path)||result.length>=1024)throw Error('全国道路shard不正');
   seen.add(path);result.push({bounds:b,manifest:path,sha256:shard.sha256,sourceUrl:source.url,sourceSha:source.sha256,revision:root.revision,coverageVerified:root.coverage_verified});
  }
 }
 return result;
}
/** Missing regional boundary cells are unknown, not empty. The initial index reader
 * deliberately rejects requests spanning several shards instead of claiming a
 * complete merged graph. National publication must be separately configured. */
export function createNationalRoadRepository(base:URL,indexSha:string,fetcher:typeof fetch=fetch){
 if(!HASH.test(indexSha)||base.origin!==globalThis.location?.origin||base.pathname!=='/road-data/'||base.search||base.hash||base.username||base.password)throw Error('全国道路配信先が不正');
 const fixed=new URL(base.href),repositories=new Map<string,ReturnType<typeof createRoadPackRepository>>();let index:Promise<Shard[]>|undefined;let retryAfter=0;const budget={spent:0};let active=0;
 async function readIndex(){const control=new AbortController(),timer=setTimeout(()=>control.abort(),25000);try{
  if(budget.spent>=50*1024*1024)throw Error('全国道路セッション容量上限');const url=new URL('index.json',fixed).href,response=await fetcher(url,{redirect:'error',signal:control.signal,credentials:'same-origin'});
  if(!response.ok||!response.body||(response.url&&response.url!==url)||Number(response.headers.get('content-length'))>MAX_INDEX_BYTES)throw Error('全国道路index取得不可');
  const reader=response.body.getReader(),chunks:Uint8Array[]=[];let size=0;
  try{for(;;){const {done,value}=await reader.read();if(done)break;size+=value.byteLength;budget.spent+=value.byteLength;if(size>MAX_INDEX_BYTES||budget.spent>50*1024*1024){await reader.cancel();throw Error('全国道路index容量超過');}chunks.push(value);}}finally{reader.releaseLock();}
  const bytes=new Uint8Array(size);let offset=0;for(const part of chunks){bytes.set(part,offset);offset+=part.length;}
  const hash=[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(n=>n.toString(16).padStart(2,'0')).join('');if(hash!==indexSha)throw Error('全国道路index SHA-256不一致');
  return parseNationalRoadIndex(JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes)));
 }finally{control.abort();clearTimeout(timer);}}
 return {async load(bounds:GraphBounds):Promise<RoadSnapshot>{if(active>=12)throw Error('全国道路取得待機上限');active++;try{
  roadBoundsKey(bounds);if(!index&&Date.now()<retryAfter)throw Error('全国道路index再取得待機中');index??=readIndex().catch(error=>{index=undefined;retryAfter=Date.now()+60000;throw error;});const entries=await index;
  const matches=entries.filter(({bounds:b})=>bounds.west>=b[0]&&bounds.south>=b[1]&&bounds.east<=b[2]&&bounds.north<=b[3]);
  if(!matches.length)throw Error('全国道路未提供範囲、または配信区画境界です。未取得は道路なしを意味しません。');
  // A deterministic provider is chosen; never retry another source to hide gaps.
  const shard=matches.sort((a,b)=>a.manifest.localeCompare(b.manifest))[0];
  let repo=repositories.get(shard.manifest);if(!repo){repo=createRoadPackRepository(new URL(shard.manifest.replace('manifest.json',''),fixed),shard.sha256,fetcher,Date.now,budget);repositories.set(shard.manifest,repo);if(repositories.size>8)repositories.delete(repositories.keys().next().value!);}
  const result=await repo.load(bounds);if(result.bundle.source.sourceDate!==shard.revision||result.bundle.sourceSha256!==shard.sourceSha||result.bundle.source.source!==shard.sourceUrl)throw Error('全国道路source hash不一致');return {...result,coverageVerified:shard.coverageVerified};
 }finally{active--;}}};
}
