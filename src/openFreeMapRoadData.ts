/** Independently implemented OpenMapTiles adapter. Tile identifiers are NOT OSM IDs.
 * Successful tiles provide observed geometry, never proof that all roads exist. */
import {VectorTile} from '@mapbox/vector-tile';
import {PbfReader} from 'pbf';
import {roadBoundsKey,type RoadSnapshot,type RoadWay} from './automaticRoadData';
import {validateReferenceGraph,type GraphBounds,type GraphNode,type GraphEdge} from './referenceGraph';
import type {RoadWayMetadata} from './referenceGraphBundle';
export const OFM_METADATA='https://tiles.openfreemap.org/planet';
export const OFM_ATTRIBUTION='© OpenMapTiles © OpenStreetMap contributors';
export const OFM_LIMITS={bytes:5*1024*1024,sessionBytes:50*1024*1024,sessionTiles:3000,tilesPerLoad:16,timeout:20000,cacheEntries:64,retry:60000} as const;
export type OpenFreeMapRoadLine={id:string;revision:string;z:14;x:number;y:number;extent:number;coordinates:[number,number][];class:string;subclass?:string;access?:string;brunnel?:string;layer?:number;oneway?:number|string};
export type OpenFreeMapTileSnapshot=RoadSnapshot&{tileLines:OpenFreeMapRoadLine[]};
export type OpenFreeMapRoadSnapshot=RoadSnapshot&{tileSnapshots:OpenFreeMapTileSnapshot[];tileLines:OpenFreeMapRoadLine[]};
const roadClasses=new Set(['motorway','trunk','primary','secondary','tertiary','minor','service','track','busway','bus_guideway','path']);
const hash=async(bytes:Uint8Array)=>Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',new Uint8Array(bytes))),v=>v.toString(16).padStart(2,'0')).join('');
export function roadTileBounds(x:number,y:number):GraphBounds{const lat=(v:number)=>Math.atan(Math.sinh(Math.PI*(1-2*v/16384)))*180/Math.PI;return {west:x/16384*360-180,east:(x+1)/16384*360-180,north:lat(y),south:lat(y+1)};}
export function roadTilesOf(b:GraphBounds):[number,number][]{roadBoundsKey(b);const x=(n:number)=>Math.floor((n+180)/360*16384),y=(n:number)=>Math.floor((1-Math.asinh(Math.tan(n*Math.PI/180))/Math.PI)/2*16384);const out:[number,number][]=[];for(let xx=x(b.west);xx<=Math.min(16383,x(b.east));xx++)for(let yy=y(b.north);yy<=Math.min(16383,y(b.south));yy++){out.push([xx,yy]);if(out.length>OFM_LIMITS.tilesPerLoad)throw Error('道路タイル取得範囲が大きすぎます。');}return out;}
export function parseRoadTileTemplate(v:unknown):{template:string;revision:string}{const o=v as {tiles?:unknown;maxzoom?:unknown};if(!o||!Array.isArray(o.tiles)||o.tiles.length!==1||typeof o.tiles[0]!=='string'||o.maxzoom!==14)throw Error('道路タイル情報が不正です。');const m=o.tiles[0].match(/^https:\/\/tiles\.openfreemap\.org\/planet\/([A-Za-z0-9_]{1,64})\/\{z\}\/\{x\}\/\{y\}\.pbf$/);if(!m)throw Error('道路タイル提供先が不正です。');return {template:o.tiles[0],revision:m[1]};}
export async function decodeOpenFreeMapRoadTile(bytes:Uint8Array,x:number,y:number,revision:string,fetchedAt:number):Promise<OpenFreeMapTileSnapshot>{
 if(bytes.length>OFM_LIMITS.bytes||!Number.isInteger(x)||!Number.isInteger(y)||x<0||x>=16384||y<0||y>=16384||!/^[A-Za-z0-9_]{1,64}$/.test(revision))throw Error('道路タイル入力が不正です。');
 const tile=new VectorTile(new PbfReader(bytes)),layer=tile.layers.transportation;if(!layer)throw Error('道路レイヤー未取得です（道路なしを意味しません）。');if(layer.length>10000)throw Error('道路要素数が上限を超えています。');
 const tileLines:OpenFreeMapRoadLine[]=[];
 const bounds=roadTileBounds(x,y),extent={...bounds},nodes:GraphNode[]=[],edges:GraphEdge[]=[],roads:RoadWay[]=[],roadWays:Record<string,RoadWayMetadata>=Object.create(null);
 for(let i=0;i<layer.length;i++){const f=layer.feature(i),p=f.properties;if(f.type!==2||!roadClasses.has(String(p.class)))continue;
 const str=(k:string)=>{if(!Object.hasOwn(p,k))return null;if(typeof p[k]!=='string'&&typeof p[k]!=='number'||typeof p[k]==='number'&&!Number.isFinite(p[k]))throw Error('道路属性の型が不正です。');if(['brunnel','subclass','access'].includes(k)&&typeof p[k]!=='string')throw Error('道路属性の型が不正です。');const v=String(p[k]);if(v!==null&&(!v||new TextEncoder().encode(v).length>100||/[\p{Cc}\p{Cf}]/u.test(v)))throw Error('道路属性が不正です。');return v;};const brunnel=str('brunnel'),rawLayer=str('layer'),nLayer=rawLayer===null?null:Number(rawLayer);if(nLayer!==null&&!Number.isFinite(nLayer)||typeof p.oneway==='number'&&!Number.isFinite(p.oneway))throw Error('道路構造属性が非有限値です。');if(brunnel!==null&&!['bridge','tunnel'].includes(brunnel))throw Error('未対応の道路構造です（地上とは判定できません）。');const supportedLayer=nLayer===null||Number.isInteger(nLayer)&&Math.abs(nLayer)<=10;
 const bridge=brunnel==='bridge'?'yes':brunnel===null?'no':null,tunnel=brunnel==='tunnel'?'yes':brunnel===null?'no':null;
 const geom=f.loadGeometry();for(let j=0;j<geom.length;j++){const line=geom[j];if(line.length<2)continue;const id=`ofm:${revision}:14:${x}:${y}:${i}:${j}`;const points=line.map((pt,k)=>{const longitude=(x+pt.x/f.extent)/16384*360-180,latitude=Math.atan(Math.sinh(Math.PI*(1-2*(y+pt.y/f.extent)/16384)))*180/Math.PI;extent.west=Math.min(extent.west,longitude);extent.east=Math.max(extent.east,longitude);extent.south=Math.min(extent.south,latitude);extent.north=Math.max(extent.north,latitude);return {osmId:`${id}:${k}`,latitude,longitude};});
 if(nodes.length+points.length>20000)throw Error('道路頂点数が上限を超えています。');nodes.push(...points);
 tileLines.push({id,revision,z:14,x,y,extent:f.extent,coordinates:points.map(p=>[p.longitude,p.latitude]),class:String(p.class),...(str("access")===null?{}:{access:str("access")!}),...(str("subclass")===null?{}:{subclass:str("subclass")!}),...(brunnel===null?{}:{brunnel}),...(nLayer===null?{}:{layer:nLayer}),...(typeof p.oneway==='number'?{oneway:p.oneway}:str('oneway')===null?{}:{oneway:str('oneway')!})});
 // OpenMapTiles omits default ground structural tags; unsupported values stay unknown.
 const observed:RoadWay&{classification:{class:string;subclass:string|null;access:string|null;oneway:string|null}}={classification:{class:String(p.class),subclass:str("subclass"),access:str("access"),oneway:str("oneway")},id,points:points.map(({latitude,longitude})=>({latitude,longitude})),layer:rawLayer,level:null,bridge,tunnel,coverage:[bounds]};roads.push(observed);
 roadWays[id]={highway:String(p.class)==='minor'?'residential':String(p.class),access:str('access'),motorVehicle:null,structureUnsupported:String(p.class)==='path'||!supportedLayer||(nLayer!==null&&nLayer!==0)||brunnel!==null};
 for(let k=0;k<points.length-1;k++){const a=points[k],b=points[k+1];if(a.longitude===b.longitude&&a.latitude===b.latitude)continue;edges.push({id:`${id}/${k}`,osmWayId:id,segmentIndex:k,from:a.osmId,to:b.osmId,mode:'road',layer:supportedLayer?nLayer:null,level:null,bridge:bridge===null?null:bridge==='yes',tunnel:tunnel===null?null:tunnel==='yes'});}
 }}
 const source=`${OFM_METADATA}/${revision}/14/${x}/${y}.pbf`;const graph=validateReferenceGraph({format:'rosenfu-reference-graph',version:1,dataset:{id:`ofm-tiles/${revision}/14/${x}/${y}`,revision,source,license:'ODbL-1.0',bounds:extent},nodes,edges});
 return {bounds,fetchedAt,coverageVerified:false,tileLines,roads,bundle:{graph,roadWays,graphSha256:await hash(new TextEncoder().encode(JSON.stringify(graph))),sourceSha256:await hash(bytes),source:{source,sourceDate:null,license:'ODbL-1.0',attribution:OFM_ATTRIBUTION}}};
}
/** Exact inclusive segment/box intersection. Both endpoints may be outside. */
export function roadFragmentIntersects(points:readonly {longitude:number;latitude:number}[],b:GraphBounds):boolean{
 for(let i=1;i<points.length;i++){const a=points[i-1],z=points[i];let lo=0,hi=1;let ok=true;
  for(const [v,d,min,max] of [[a.longitude,z.longitude-a.longitude,b.west,b.east],[a.latitude,z.latitude-a.latitude,b.south,b.north]]){if(d===0){if(v<min||v>max){ok=false;break;}}else{const t0=(min-v)/d,t1=(max-v)/d;lo=Math.max(lo,Math.min(t0,t1));hi=Math.min(hi,Math.max(t0,t1));if(lo>hi){ok=false;break;}}}
  if(ok)return true;
 }return false;
}
export function createOpenFreeMapRoadRepository(fetcher:typeof fetch=fetch,now:()=>number=Date.now){
 type Task={promise:Promise<OpenFreeMapTileSnapshot>;users:Set<symbol>;controller:AbortController};
 let template:Promise<{template:string;revision:string}>|undefined;
 let metadataRetryAfter=0,usedBytes=0,usedTiles=0,activeLoads=0;let metadataController:AbortController|undefined;let metadataReady=false;
 let queue:Promise<unknown>=Promise.resolve();
 const cache=new Map<string,OpenFreeMapTileSnapshot>(),pending=new Map<string,Task>(),failures=new Map<string,number>();
 const cancelled=()=>new DOMException('道路取得を取り消しました。','AbortError');
 const check=(signal:AbortSignal)=>{if(signal.aborted)throw cancelled();};
 // A load may stop waiting without cancelling work used by another load.
 async function wait<T>(promise:Promise<T>,signal:AbortSignal):Promise<T>{
  check(signal);let abort:()=>void=()=>{};
  const stopped=new Promise<never>((_,reject)=>{abort=()=>reject(cancelled());signal.addEventListener('abort',abort,{once:true});});
  try{return await Promise.race([promise,stopped]);}finally{signal.removeEventListener('abort',abort);}
 }
 async function request(url:string,shared?:AbortSignal){
  const controller=new AbortController(),abort=()=>controller.abort();
  shared?.addEventListener('abort',abort,{once:true});if(shared?.aborted)controller.abort();
  const timer=setTimeout(abort,OFM_LIMITS.timeout);
  try{
   check(controller.signal);
   const r=await fetcher(url,{signal:controller.signal,credentials:'omit',referrerPolicy:'no-referrer'});
   check(controller.signal);
   if((r.url&&new URL(r.url).origin!=='https://tiles.openfreemap.org')||!r.ok||!r.body)throw Error(`道路タイルを取得できません（HTTP ${r.status}）。`);
   const reader=r.body.getReader(),chunks:Uint8Array[]=[];let size=0;
   const cancelReader=()=>{void reader.cancel().catch(()=>undefined);};controller.signal.addEventListener('abort',cancelReader,{once:true});
   try{for(;;){check(controller.signal);const {done,value}=await reader.read();check(controller.signal);if(done)break;size+=value.length;usedBytes+=value.length;if(size>OFM_LIMITS.bytes||usedBytes>OFM_LIMITS.sessionBytes){await reader.cancel();throw Error('道路データ取得容量上限です。');}chunks.push(value);}}
   finally{controller.signal.removeEventListener('abort',cancelReader);reader.releaseLock();}
   const result=new Uint8Array(size);let off=0;for(const c of chunks){result.set(c,off);off+=c.length;}return result;
  }finally{clearTimeout(timer);shared?.removeEventListener('abort',abort);}
 }
 function metadata(){
  if(metadataController?.signal.aborted&&!metadataReady)template=undefined;
  if(!template){if(now()<metadataRetryAfter)return Promise.reject(Error('道路タイル情報再試行待機中です。'));
   const ctrl=new AbortController();metadataController=ctrl;metadataReady=false;
   const task=request(OFM_METADATA,ctrl.signal).then(b=>{const result=parseRoadTileTemplate(JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(b)));check(ctrl.signal);metadataReady=true;return result;}).catch(e=>{if(template===task){template=undefined;if(!ctrl.signal.aborted)metadataRetryAfter=now()+OFM_LIMITS.retry;}throw e;});template=task;
  }return template;
 }
 function acquire(x:number,y:number,meta:{template:string;revision:string},user:symbol,owned:Set<Task>):Promise<OpenFreeMapTileSnapshot>{
  const key=`${meta.revision}/${x}/${y}`,hit=cache.get(key);if(hit)return Promise.resolve(hit);
  let task=pending.get(key);
  if(task?.controller.signal.aborted){if(pending.get(key)===task)pending.delete(key);task=undefined;}
  if(!task){
   if(pending.size>=32)return Promise.reject(Error('道路タイル待機上限です。'));
   if(now()-(failures.get(key)??-Infinity)<OFM_LIMITS.retry)return Promise.reject(Error('道路タイル再試行待機中です。'));
   const controller=new AbortController();const created:Task={controller,users:new Set(),promise:Promise.resolve(null as unknown as OpenFreeMapTileSnapshot)};
   const work=queue.then(async()=>{
    check(controller.signal); // queued work with no subscribers performs no fetch
    if(usedTiles>=OFM_LIMITS.sessionTiles||usedBytes>=OFM_LIMITS.sessionBytes)throw Error('道路タイル取得予算上限です。');
    usedTiles++;const bytes=await request(meta.template.replace('{z}','14').replace('{x}',String(x)).replace('{y}',String(y)),controller.signal);check(controller.signal);
    const snapshot=await decodeOpenFreeMapRoadTile(bytes,x,y,meta.revision,now());check(controller.signal);return snapshot;
   }).then(snapshot=>{cache.set(key,snapshot);while(cache.size>OFM_LIMITS.cacheEntries)cache.delete(cache.keys().next().value!);return snapshot;}).catch(e=>{
    if(!controller.signal.aborted){failures.set(key,now());while(failures.size>128)failures.delete(failures.keys().next().value!);}throw e;
   }).finally(()=>{if(pending.get(key)===created)pending.delete(key);});
   created.promise=work;task=created;pending.set(key,task);queue=work.catch(()=>undefined);
  }
  task.users.add(user);owned.add(task);return task.promise;
 }
 return {async load(bounds:GraphBounds,signal?:AbortSignal):Promise<OpenFreeMapRoadSnapshot>{
  const tiles=roadTilesOf(bounds),user=Symbol('road-consumer'),owned=new Set<Task>(),controller=new AbortController();
  const abort=()=>controller.abort();signal?.addEventListener('abort',abort,{once:true});if(signal?.aborted)abort();
  const timer=setTimeout(abort,25000);activeLoads++;
  try{
   check(controller.signal);const meta=await wait(metadata(),controller.signal);check(controller.signal);
   const snapshots=await wait(Promise.all(tiles.map(([x,y])=>acquire(x,y,meta,user,owned))),controller.signal);check(controller.signal);
   const wanted=new Set(snapshots.flatMap(s=>s.roads.filter(r=>roadFragmentIntersects(r.points,bounds)).map(r=>r.id)));
   const edges=snapshots.flatMap(s=>s.bundle.graph.edges.filter(e=>wanted.has(e.osmWayId))),nodeIds=new Set(edges.flatMap(e=>[e.from,e.to]));
   const nodes=snapshots.flatMap(s=>s.bundle.graph.nodes.filter(n=>nodeIds.has(n.osmId)));
   const extent=snapshots.reduce((b,s)=>({west:Math.min(b.west,s.bundle.graph.dataset.bounds.west),east:Math.max(b.east,s.bundle.graph.dataset.bounds.east),south:Math.min(b.south,s.bundle.graph.dataset.bounds.south),north:Math.max(b.north,s.bundle.graph.dataset.bounds.north)}),{...bounds});
   const revision=snapshots[0].bundle.graph.dataset.revision;
   const graph=validateReferenceGraph({format:'rosenfu-reference-graph',version:1,dataset:{id:`ofm-tiles/${revision}`,revision,source:OFM_METADATA,license:'ODbL-1.0',bounds:extent},nodes,edges});
   const graphSha256=await hash(new TextEncoder().encode(JSON.stringify(graph))),sourceSha256=await hash(new TextEncoder().encode(snapshots.map(s=>s.bundle.sourceSha256).join(',')));check(controller.signal);
   return {bounds:{...bounds},fetchedAt:now(),coverageVerified:false,tileSnapshots:snapshots,tileLines:snapshots.flatMap(s=>s.tileLines),roads:snapshots.flatMap(s=>s.roads),bundle:{graph,roadWays:Object.fromEntries(snapshots.flatMap(s=>Object.entries(s.bundle.roadWays??{}).filter(([id])=>wanted.has(id)))),graphSha256,sourceSha256,source:{source:OFM_METADATA,sourceDate:null,license:'ODbL-1.0',attribution:OFM_ATTRIBUTION}}};
  }finally{
   activeLoads--;if(activeLoads===0&&!metadataReady)metadataController?.abort();
   clearTimeout(timer);signal?.removeEventListener('abort',abort);
   for(const task of owned){task.users.delete(user);if(task.users.size===0)task.controller.abort();}
  }
 }};
}
