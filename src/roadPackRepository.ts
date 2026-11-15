import {parseRoadResponse,roadBoundsKey,type RoadSnapshot} from './automaticRoadData';
import type {GraphBounds} from './referenceGraph';

export const ROAD_PACK_LIMITS={bytes:5*1024*1024,sessionBytes:50*1024*1024,cacheEntries:128,pendingEntries:12,timeoutMs:25000,retryMs:60000,cells:4096,ways:10000,coords:100000} as const;
const CLASSES='motorway motorway_link trunk trunk_link primary primary_link secondary secondary_link tertiary tertiary_link unclassified residential living_street service'.split(' ');
const TAGS=['highway','bridge','tunnel','layer','level','access','motor_vehicle'] as const;
const HASH=/^[a-f0-9]{64}$/;
const GRID=/^(-?(?:0|[1-9]\d*))_(-?(?:0|[1-9]\d*))$/;
type Box=[number,number,number,number];
type CellEntry={file:string;bounds:Box;sha256:string;bytes:number;ways:number};
type Manifest={revision:string;source:{url:string;file_sha256:string};cells:Map<string,CellEntry>};
type Way={type:'way';id:string;nodes:string[];geometry:{lon:number;lat:number}[];tags:Record<string,string>};
function fail(message:string):never{throw Error(`道路パック：${message}`);}
function object(value:unknown):Record<string,unknown>{if(!value||typeof value!=='object'||Array.isArray(value))fail('形式が不正です。');return value as Record<string,unknown>;}
function string(value:unknown,max=4096):string{if(typeof value!=='string'||!value.length||value.length>max)fail('文字列が不正です。');return value;}
function integer(value:unknown,max:number,min=0):number{if(typeof value!=='number'||!Number.isSafeInteger(value)||value<min||value>max)fail('件数またはサイズが上限外です。');return value;}
function hash(value:unknown):string{const v=string(value,64);if(!HASH.test(v))fail('SHA-256が不正です。');return v;}
function id(value:unknown):string{const v=string(value,19);if(!/^[1-9]\d*$/.test(v)||BigInt(v)>=2n**63n)fail('OSM IDが不正です。');return v;}
function box(value:unknown):Box{if(!Array.isArray(value)||value.length!==4||!value.every(v=>typeof v==='number'&&Number.isFinite(v)))fail('範囲が不正です。');const [w,s,e,n]=value;if(w< -180||e>180||s< -85||n>85||w>=e||s>=n)fail('範囲が不正です。');return [w,s,e,n];}
function gridBox(key:string):Box{const m=GRID.exec(key);if(!m)fail('セルキーが不正です。');const x=Number(m[1]),y=Number(m[2]);if(!Number.isSafeInteger(x)||!Number.isSafeInteger(y)||x< -9000||x>=9000||y< -4250||y>=4250)fail('セルキーが範囲外です。');return [x,y,x+1,y+1].map(n=>Number((n*.02).toFixed(10))) as Box;}
function equalBox(a:Box,b:Box):boolean{return a.every((v,i)=>v===b[i]);}
async function digest(bytes:Uint8Array):Promise<string>{return [...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes as Uint8Array<ArrayBuffer>))].map(n=>n.toString(16).padStart(2,'0')).join('');}
function decode(bytes:Uint8Array):unknown{return JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes));}
function parseManifest(value:unknown):Manifest{
 const m=object(value),source=object(m.source),entries=object(m.cells);
 if(m.format!=='rosenfu-road-pack'||m.version!==1||m.cell_degrees!==.02||m.license!=='ODbL-1.0'||m.excluded_area!=='yes')fail('未対応のmanifestです。');
 if(!Array.isArray(m.highway_classes)||JSON.stringify(m.highway_classes)!==JSON.stringify(CLASSES))fail('道路種別の契約が一致しません。');
 string(m.attribution);string(m.license_url);string(m.coverage_meaning);box(m.requested_bounds);
 const revision=string(m.revision,256);
 if(!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/.test(revision)||!Number.isFinite(Date.parse(revision))||new Date(revision).toISOString()!==revision.replace(/(?<!\.\d{3})Z$/,'.000Z'))fail('revisionは有効なUTC ISO日時が必要です。');
 const url=string(source.url);hash(source.file_sha256);hash(source.xml_sha256);hash(source.coverage_sha256);object(source.coverage_geojson);if(source.parent_pbf_sha256!==undefined)hash(source.parent_pbf_sha256);
 const keys=Object.keys(entries);if(!keys.length||keys.length>ROAD_PACK_LIMITS.cells)fail('manifestセル数が上限外です。');
 const cells=new Map<string,CellEntry>();
 for(const key of keys){const entry=object(entries[key]),bounds=box(entry.bounds);if(entry.file!==`${key}.json`||!equalBox(bounds,gridBox(key)))fail('セルのファイル名または範囲が不正です。');cells.set(key,{file:`${key}.json`,bounds,sha256:hash(entry.sha256),bytes:integer(entry.bytes,ROAD_PACK_LIMITS.bytes,1),ways:integer(entry.ways,ROAD_PACK_LIMITS.ways)});}
 return {revision,source:{url,file_sha256:(source.parent_pbf_sha256??source.file_sha256) as string},cells};
}
function parseCell(value:unknown,entry:CellEntry,revision:string):Way[]{
 const cell=object(value);if(cell.format!=='rosenfu-road-cell'||cell.version!==1||cell.revision!==revision||!equalBox(box(cell.bounds),entry.bounds)||!Array.isArray(cell.ways)||cell.ways.length!==entry.ways)fail('セルがmanifestと一致しません。');
 const seen=new Set<string>();let coords=0;
 return cell.ways.map(raw=>{const w=object(raw),wayId=id(w.id),tags=object(w.tags);if(seen.has(wayId))fail('セル内に道路IDの重複があります。');seen.add(wayId);
  if(!Array.isArray(w.node_ids)||!Array.isArray(w.coordinates)||w.node_ids.length!==w.coordinates.length||w.node_ids.length<2)fail('道路形状が欠損しています。');coords+=w.node_ids.length;if(coords>ROAD_PACK_LIMITS.coords)fail('座標数が上限外です。');
  const nodes=w.node_ids.map(id),geometry=w.coordinates.map(raw=>{if(!Array.isArray(raw)||raw.length!==2||!raw.every(v=>typeof v==='number'&&Number.isFinite(v))||Math.abs(raw[0])>180||Math.abs(raw[1])>85)fail('道路座標が不正です。');return {lon:raw[0] as number,lat:raw[1] as number};});
  const clean:Record<string,string>={};for(const key of TAGS){if(!Object.hasOwn(tags,key))fail('属性が欠損しています。');if(tags[key]!==null){if(typeof tags[key]!=='string'||tags[key].length>4096)fail('属性が不正です。');clean[key]=tags[key];}}
  if(!CLASSES.includes(clean.highway))fail('契約外の道路種別です。');
  return {type:'way',id:wayId,nodes,geometry,tags:clean};
 });
}

/** Requires a build-pinned manifest hash. This only certifies extraction from that
 * source snapshot, never completeness of roads in the real world. No fallback host. */
export function createRoadPackRepository(base:URL,expectedSha256:string,fetcher:typeof fetch=fetch,now:()=>number=Date.now){
 hash(expectedSha256);
 const origin=globalThis.location?.origin;
 if(!origin||base.origin!==origin||base.pathname!=='/road-data/'||base.search||base.hash||base.username||base.password)fail('同一originの固定配信先が必要です。');
 // Copy the caller's mutable URL; no later URL mutation can redirect requests.
 const fixedBase=base.href;
 const failures=new Map<string,number>();
 let manifest:Manifest|undefined,spent=0,queue:Promise<unknown>=Promise.resolve();
 const cells=new Map<string,Way[]>(),cache=new Map<string,RoadSnapshot>(),pending=new Map<string,Promise<RoadSnapshot>>();
 function remember<T>(store:Map<string,T>,key:string,value:T){store.delete(key);store.set(key,value);while(store.size>ROAD_PACK_LIMITS.cacheEntries)store.delete(store.keys().next().value!);}
 async function download(file:string):Promise<Uint8Array>{
  if(spent>=ROAD_PACK_LIMITS.sessionBytes)fail('セッション取得容量の上限です。');
  const url=new URL(file,fixedBase).href,controller=new AbortController(),timer=setTimeout(()=>controller.abort(),ROAD_PACK_LIMITS.timeoutMs);
  try{const response=await fetcher(url,{signal:controller.signal,redirect:'error',credentials:'same-origin'});
   if(!response.ok)fail(`取得できません（HTTP ${response.status}）。`);
   if(response.url&&response.url!==url)fail('転送された応答は使用できません。');
   const header=response.headers.get('content-length');if(header!==null&&(!/^\d+$/.test(header)||Number(header)>ROAD_PACK_LIMITS.bytes||Number(header)>ROAD_PACK_LIMITS.sessionBytes-spent))fail('応答サイズが上限外です。');
   if(!response.body)fail('応答本文がありません。');const reader=response.body.getReader(),chunks:Uint8Array[]=[];let size=0;
   try{for(;;){const {done,value}=await reader.read();if(done)break;spent+=value.byteLength;size+=value.byteLength;if(size>ROAD_PACK_LIMITS.bytes||spent>ROAD_PACK_LIMITS.sessionBytes){await reader.cancel();fail('取得容量が上限を超えました。');}chunks.push(value);}}finally{reader.releaseLock();}
   const result=new Uint8Array(size);let offset=0;for(const chunk of chunks){result.set(chunk,offset);offset+=chunk.length;}return result;
  }finally{controller.abort();clearTimeout(timer);}
 }
 async function loadNow(bounds:GraphBounds):Promise<RoadSnapshot>{
  if(!manifest){const bytes=await download('manifest.json');if(await digest(bytes)!==expectedSha256)fail('manifestのSHA-256が一致しません。');manifest=parseManifest(decode(bytes));}
  const keys:string[]=[];
  const grid=(n:number)=>{const x=n/.02,r=Math.round(x);return Math.abs(x-r)<1e-9?r:x;};
  for(let x=Math.floor(grid(bounds.west));x<Math.ceil(grid(bounds.east));x++)for(let y=Math.floor(grid(bounds.south));y<Math.ceil(grid(bounds.north));y++)keys.push(`${x}_${y}`);
  if(!keys.length||keys.some(key=>!manifest!.cells.has(key)))fail('必要な道路セルが未提供です。');
  const ways=new Map<string,Way>();let coords=0;
  for(const key of keys){let records=cells.get(key);if(!records){const entry=manifest.cells.get(key)!,bytes=await download(entry.file);if(bytes.length!==entry.bytes||await digest(bytes)!==entry.sha256)fail('セルのサイズまたはSHA-256が一致しません。');records=parseCell(decode(bytes),entry,manifest.revision);remember(cells,key,records);}
   for(const way of records){const old=ways.get(way.id);if(old){if(JSON.stringify(old)!==JSON.stringify(way))fail('セル間の道路形状が矛盾しています。');continue;}coords+=way.nodes.length;if(ways.size>=ROAD_PACK_LIMITS.ways||coords>ROAD_PACK_LIMITS.coords)fail('結合道路の上限です。');ways.set(way.id,way);}
  }
  return parseRoadResponse({osm3s:{timestamp_osm_base:manifest.revision},elements:[...ways.values()]},bounds,now(),{source:manifest.source.url,sourceSha256:manifest.source.file_sha256});
 }
 return {async load(request:GraphBounds):Promise<RoadSnapshot>{
  const bounds={...request},key=roadBoundsKey(bounds),hit=cache.get(key);if(hit)return hit;
  if((failures.get(key)??0)>now())fail("再試行待機中です。");const existing=pending.get(key);if(existing)return existing;
  if(pending.size>=ROAD_PACK_LIMITS.pendingEntries)fail('取得待機の上限です。');
  const task=queue.then(()=>loadNow(bounds)).then(snapshot=>{remember(cache,key,snapshot);return snapshot;}).catch(error=>{remember(failures,key,now()+ROAD_PACK_LIMITS.retryMs);throw error;}).finally(()=>pending.delete(key));
  pending.set(key,task);queue=task.catch(()=>undefined);return task;
 }};
}
