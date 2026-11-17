import type {TileRoadLine} from './ofmBoundaryContinuations';
// Road reference metadata only; not a completed environmental construction assessment.
export type RoadClassification={class:string;subclass:string|null;access:string|null;oneway?:string|null};
export type RoadWay={classification?:RoadClassification;coverage?:GraphBounds[];id:string;points:{latitude:number;longitude:number}[];layer:string|null;level:string|null;bridge:string|null;tunnel:string|null};
import {validateReferenceGraph, type GraphBounds, type GraphNode, type GraphEdge} from './referenceGraph';
import type {ReferenceGraphBundle, RoadWayMetadata} from './referenceGraphBundle';

export const ROAD_ENDPOINT='https://overpass-api.de/api/interpreter';
export const ROAD_LIMITS={spanDegrees:0.04,bytes:5*1024*1024,ways:10000,cacheEntries:128,pendingEntries:12,minIntervalMs:1000,ttlMs:86400000,retryMs:60000,timeoutMs:25000} as const;
export type RoadSnapshot={tileLines?:(TileRoadLine&{extent:number})[];tileSnapshots?:RoadSnapshot[];coverageVerified?:boolean;bounds:GraphBounds;fetchedAt:number;bundle:ReferenceGraphBundle;roads:RoadWay[]};
/** Overpass snapshots have no vector-tile lines; preserve the complete OFM snapshot. */
export function roadTileSnapshots(snapshot:RoadSnapshot|undefined){
 return (snapshot?.tileSnapshots??[]).filter((tile):tile is RoadSnapshot&{tileLines:NonNullable<RoadSnapshot['tileLines']>}=>Array.isArray(tile.tileLines));
}
export type RoadDataState={status:'unavailable'|'loading'|'ready'|'error';snapshot?:RoadSnapshot;message:string};
const classes='motorway|motorway_link|trunk|trunk_link|primary|primary_link|secondary|secondary_link|tertiary|tertiary_link|unclassified|residential|living_street|service';
export function roadBoundsKey(b:GraphBounds):string{
 if(!Object.values(b).every(Number.isFinite)||b.west < -180||b.east>180||b.south < -85||b.north>85||b.west>=b.east||b.south>=b.north||b.east-b.west>ROAD_LIMITS.spanDegrees+1e-9||b.north-b.south>ROAD_LIMITS.spanDegrees+1e-9)throw Error('道路取得範囲が上限を超えています。地図を拡大してください。');
 return [b.south,b.west,b.north,b.east].join(',');
}
function record(v:unknown):Record<string,unknown>{if(!v||typeof v!=='object'||Array.isArray(v))throw Error('道路応答の形式が不正です。');return v as Record<string,unknown>;}
function id(v:unknown):string{if(typeof v==='string'&&/^[1-9]\d{0,19}$/.test(v))return v;if(typeof v!=='number'||!Number.isSafeInteger(v)||v<=0)throw Error('道路IDが不正です。');return String(v);}
async function digest(value:string):Promise<string>{return Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(value))),b=>b.toString(16).padStart(2,'0')).join('');}
/** OSM geometry is a reference snapshot, never a guarantee that every physical road exists in OSM.
 * Original structural tags survive in GraphEdge; absent OSM tags are not converted to metres. */
export async function parseRoadResponse(value:unknown,bounds:GraphBounds,fetchedAt:number,metadata?:{source:string;sourceSha256:string}):Promise<RoadSnapshot>{
 roadBoundsKey(bounds);if(metadata&&(!/^https:\/\//.test(metadata.source)||metadata.source.length>2048||!/^([a-f0-9]{64})$/.test(metadata.sourceSha256)))throw Error('道路出典が不正です。');const root=record(value),meta=record(root.osm3s);
 if(root.remark!==undefined||typeof meta.timestamp_osm_base!=='string'||!Number.isFinite(Date.parse(meta.timestamp_osm_base))||!Array.isArray(root.elements)||root.elements.length>ROAD_LIMITS.ways)throw Error('道路応答が未完了または上限超過です。');
 const nodes=new Map<string,GraphNode>(),edges:GraphEdge[]=[],roadWays:Record<string,RoadWayMetadata>=Object.create(null),seen=new Set<string>();
 const extent={...bounds};const roads:RoadWay[]=[];
 for(const raw of root.elements){
  const w=record(raw);if(w.type!=='way')throw Error('未対応の道路要素です。');const way=id(w.id),tags=record(w.tags);
  if(seen.has(way)||!Array.isArray(w.nodes)||!Array.isArray(w.geometry)||w.nodes.length!==w.geometry.length||w.nodes.length<2)throw Error('道路形状が欠損しています。');seen.add(way);
  if(Object.keys(tags).length>256||Object.values(tags).some(v=>typeof v!=='string'||v.length>4096))throw Error('道路属性が不正です。');
  const tag=(k:string)=>typeof tags[k]==='string'?tags[k] as string:null;
  const highway=tag('highway');if(!highway||!classes.split('|').includes(highway)||tags.area==='yes')continue;
  const layer=tag('layer'),level=tag('level');
  const numericLayer=layer!==null&&/^[+-]?\d+$/.test(layer)&&Math.abs(Number(layer))<=100?Number(layer):null;
  const flag=(k:string)=>tags[k]===undefined?null:tags[k]==='no'?false:true;
  const bridge=flag('bridge'),tunnel=flag('tunnel');
  const unsupported=bridge===true||tunnel===true||(layer!==null&&numericLayer!==0)||(level!==null&&!/^[+-]?0(?:\.0+)?$/.test(level));
  roadWays[way]={highway,access:tag('access'),motorVehicle:tag('motor_vehicle'),structureUnsupported:unsupported};
  const points=w.geometry.map((raw,i)=>{const p=record(raw),node={osmId:id((w.nodes as unknown[])[i]),latitude:p.lat as number,longitude:p.lon as number};
   if(!Number.isFinite(node.latitude)||!Number.isFinite(node.longitude)||Math.abs(node.latitude)>85||Math.abs(node.longitude)>180)throw Error('道路座標が不正です。');
   const previous=nodes.get(node.osmId);if(previous&&(previous.latitude!==node.latitude||previous.longitude!==node.longitude))throw Error('道路ノードの座標が競合しています。');
   nodes.set(node.osmId,node);extent.west=Math.min(extent.west,node.longitude);extent.east=Math.max(extent.east,node.longitude);extent.south=Math.min(extent.south,node.latitude);extent.north=Math.max(extent.north,node.latitude);return node;});
  roads.push({id:way,points:points.map(({latitude,longitude})=>({latitude,longitude})),layer,level,bridge:tag('bridge'),tunnel:tag('tunnel')});
  for(let i=0;i<points.length-1;i++){const a=points[i],b=points[i+1];if(a.osmId===b.osmId||(a.latitude===b.latitude&&a.longitude===b.longitude))continue;
   edges.push({id:`road/${way}/${i}`,osmWayId:way,segmentIndex:i,from:a.osmId,to:b.osmId,mode:'road',layer:numericLayer,level,bridge,tunnel});}
 }
 const sourceDate=meta.timestamp_osm_base,source=metadata?.source??ROAD_ENDPOINT;
 const graph=validateReferenceGraph({format:'rosenfu-reference-graph',version:1,dataset:{id:`osm-roads/${roadBoundsKey(bounds)}`,revision:sourceDate,source,license:'ODbL-1.0',bounds:extent},nodes:[...nodes.values()],edges});
 const graphSha256=await digest(JSON.stringify(graph)),sourceSha256=metadata?.sourceSha256??await digest(JSON.stringify(root));
 return {bounds:{...bounds},fetchedAt,roads,bundle:{graph,roadWays,graphSha256,sourceSha256,source:{source,sourceDate,license:'ODbL-1.0',attribution:'© OpenStreetMap contributors'}}};
}
/** One queue per repository: coalesce identical requests and never hammer the public service.
 * Failure and valid empty results are distinct; failed entries have a retry cooldown. */
export function createRoadRepository(fetcher:typeof fetch=fetch,now:()=>number=Date.now){
 const cache=new Map<string,RoadSnapshot>(),pending=new Map<string,Promise<RoadSnapshot>>(),failures=new Map<string,{time:number;error:Error}>();let queue:Promise<unknown>=Promise.resolve();let nextRequestAt=0,blockedUntil=0;
 async function download(bounds:GraphBounds){
  if(now()<blockedUntil)throw Error('道路提供元の再試行待機中です。');
  const wait=Math.max(0,nextRequestAt-now());if(wait)await new Promise(resolve=>setTimeout(resolve,wait));nextRequestAt=now()+ROAD_LIMITS.minIntervalMs;
  const controller=new AbortController(),timer=setTimeout(()=>controller.abort(),ROAD_LIMITS.timeoutMs);
  try{
   const query=`[out:json][timeout:20];way["highway"~"^(${classes})$"]["area"!="yes"](${roadBoundsKey(bounds)});out body geom;`;
   const response=await fetcher(ROAD_ENDPOINT,{method:'POST',body:new URLSearchParams({data:query}),signal:controller.signal});
   if(!response.ok)throw Error(`道路データを取得できません（HTTP ${response.status}）。`);
   if(Number(response.headers.get('content-length'))>ROAD_LIMITS.bytes||!response.body)throw Error('道路応答のサイズが不正です。');
   const reader=response.body.getReader(),chunks:Uint8Array[]=[];let size=0;
   try{for(;;){const {done,value}=await reader.read();if(done)break;size+=value.byteLength;if(size>ROAD_LIMITS.bytes){await reader.cancel();throw Error('道路データが取得上限を超えています。');}chunks.push(value);}}finally{reader.releaseLock();}
   const bytes=new Uint8Array(size);let offset=0;for(const chunk of chunks){bytes.set(chunk,offset);offset+=chunk.length;}
   return await parseRoadResponse(JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes)),bounds,now());
  }finally{clearTimeout(timer);}
 }
 return {async load(bounds:GraphBounds):Promise<RoadSnapshot>{
  const key=roadBoundsKey(bounds),hit=cache.get(key);if(hit&&now()-hit.fetchedAt<ROAD_LIMITS.ttlMs)return hit;
  const existing=pending.get(key);if(existing)return existing;
  if(pending.size>=ROAD_LIMITS.pendingEntries)throw Error('道路取得の待機上限です。取得完了後に再試行します。');
  const failure=failures.get(key);if(failure&&now()-failure.time<ROAD_LIMITS.retryMs)throw failure.error;
  const task=queue.then(()=>download(bounds)).then(snapshot=>{cache.delete(key);cache.set(key,snapshot);while(cache.size>ROAD_LIMITS.cacheEntries)cache.delete(cache.keys().next().value!);failures.delete(key);return snapshot;}).catch((reason:unknown)=>{const error=reason instanceof Error?reason:Error('道路取得に失敗しました。');blockedUntil=now()+ROAD_LIMITS.retryMs;failures.set(key,{time:now(),error});while(failures.size>ROAD_LIMITS.cacheEntries)failures.delete(failures.keys().next().value!);throw error;}).finally(()=>pending.delete(key));
  pending.set(key,task);queue=task.catch(()=>undefined);return task;
 }};
}
