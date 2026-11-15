import {PMTiles,Compression,SharedPromiseCache,type Source,type DecompressFunc} from 'pmtiles';
import {VectorTile} from '@mapbox/vector-tile';
import {PbfReader} from 'pbf';
import type {GraphBounds} from './referenceGraph';
import type {WaterPolygon} from './environmentData';
export const WATER_ENDPOINT='https://cyberjapandata.gsi.go.jp/xyz/optimal_bvmap-v1/optimal_bvmap-v1.pmtiles';
const MAX_BYTES=5*1024*1024;
async function limited(stream:ReadableStream<Uint8Array>):Promise<ArrayBuffer>{const reader=stream.getReader(),chunks:Uint8Array[]=[];let size=0;try{for(;;){const {done,value}=await reader.read();if(done)break;size+=value.length;if(size>MAX_BYTES){await reader.cancel();throw Error('水域応答が上限を超えています。');}chunks.push(value);}}finally{reader.releaseLock();}const out=new Uint8Array(size);let at=0;for(const c of chunks){out.set(c,at);at+=c.length;}return out.buffer;}
const decompress:DecompressFunc=async(buf,compression)=>{if(compression===Compression.None){if(buf.byteLength>MAX_BYTES)throw Error('水域応答が上限を超えています。');return buf;}if(compression!==Compression.Gzip)throw Error('未対応の水域圧縮形式です。');return limited(new Blob([buf]).stream().pipeThrough(new DecompressionStream('gzip')));};
/** Fixed-source, bounded HTTP Range requests. A server returning its full archive is rejected. */
export function createWaterSource(fetcher:typeof fetch=fetch):Source{return {getKey:()=>WATER_ENDPOINT,async getBytes(offset,length,signal,etag){
 if(!Number.isSafeInteger(offset)||offset<0||!Number.isSafeInteger(length)||length<=0||length>MAX_BYTES)throw Error('水域取得範囲が不正です。');
 const controller=new AbortController(),timer=setTimeout(()=>controller.abort(),25000);const abort=()=>controller.abort();signal?.addEventListener('abort',abort,{once:true});if(signal?.aborted)abort();
 try{const r=await fetcher(WATER_ENDPOINT,{headers:{Range:`bytes=${offset}-${offset+length-1}`},signal:controller.signal});
 if(r.status!==206||!r.body)throw Error(`水域データを取得できません（HTTP ${r.status}）。`);
 const match=/^bytes (\d+)-(\d+)\/(\d+)$/.exec(r.headers.get('content-range')??'');if(!match||Number(match[1])!==offset||Number(match[2])!==offset+length-1)throw Error('水域Range応答が不正です。');
 if(etag&&r.headers.get('etag')!==etag)throw Error('水域アーカイブが更新されました。再読込してください。');
 const data=await limited(r.body);if(data.byteLength!==length)throw Error('水域データが欠損しています。');return {data,etag:r.headers.get('etag')??undefined};
 }finally{clearTimeout(timer);signal?.removeEventListener('abort',abort);}
}};}
export function waterTileBounds(x:number,y:number,z:number):GraphBounds{const n=2**z,lat=(v:number)=>Math.atan(Math.sinh(Math.PI*(1-2*v/n)))*180/Math.PI;return {west:x/n*360-180,east:(x+1)/n*360-180,north:lat(y),south:lat(y+1)};}
export function decodeWaterTile(data:ArrayBuffer,x:number,y:number,z:number):WaterPolygon[]{
 if(data.byteLength>MAX_BYTES)throw Error('水域タイルが上限を超えています。');
 const tile=new VectorTile(new PbfReader(new Uint8Array(data))),layer=tile.layers.WA;
 // Empty/missing archive tiles are not evidence of land. AdmArea identifies a populated land tile.
 if(!layer&&!tile.layers.AdmArea)throw Error('このタイルの水域カバレッジを確認できません。');
 const water:WaterPolygon[]=[];let count=0;
 for(let i=0;i<(layer?.length??0);i++){const f=layer.feature(i);if(f.type!==3)throw Error('水域面の形式が不正です。');const geometry=f.toGeoJSON(x,y,z).geometry;
  const polygons=geometry.type==='Polygon'?[geometry.coordinates]:geometry.type==='MultiPolygon'?geometry.coordinates:null;if(!polygons)throw Error('水域形状が不正です。');
  for(const polygon of polygons){const rings=polygon.map(ring=>ring.map(([longitude,latitude])=>{if(++count>100000||!Number.isFinite(longitude)||!Number.isFinite(latitude)||Math.abs(longitude)>180||Math.abs(latitude)>85)throw Error('水域座標が不正です。');return {longitude,latitude};}));water.push({rings});}
 }return water;
}
export function createWaterRepository(fetcher:typeof fetch=fetch){
 const cache=new SharedPromiseCache(32,true,decompress),archive=new PMTiles(createWaterSource(fetcher),cache,decompress);
 const tiles=new Map<string,{at:number;water:WaterPolygon[]}>(),pending=new Map<string,Promise<WaterPolygon[]>>();let queue:Promise<unknown>=Promise.resolve();
 async function tile(x:number,y:number,z:number){const key=`${z}/${x}/${y}`,hit=tiles.get(key);if(hit&&Date.now()-hit.at<86400000)return hit.water;const old=pending.get(key);if(old)return old;
  if(pending.size>=32)throw Error('水域取得の待機上限です。');
  const task=queue.then(async()=>{const result=await archive.getZxy(z,x,y);if(!result)throw Error('水域タイル未提供です。陸地とは判定しません。');const water=decodeWaterTile(result.data,x,y,z);tiles.set(key,{at:Date.now(),water});while(tiles.size>32)tiles.delete(tiles.keys().next().value!);return water;}).finally(()=>pending.delete(key));pending.set(key,task);queue=task.catch(()=>undefined);return task;
 }
 return {async load(b:GraphBounds){const z=14,n=2**z,tx=(v:number)=>Math.floor((v+180)/360*n),ty=(v:number)=>Math.floor((1-Math.asinh(Math.tan(v*Math.PI/180))/Math.PI)/2*n);
  const minX=tx(b.west),maxX=tx(b.east),minY=ty(b.north),maxY=ty(b.south);if(!Object.values(b).every(Number.isFinite)||b.west>=b.east||b.south>=b.north||b.south<20||b.north>46||b.west<122||b.east>154||(maxX-minX+1)*(maxY-minY+1)>16)throw Error('水域取得範囲外または上限超過です。');
  const coverage:GraphBounds[]=[],water:WaterPolygon[]=[];for(let y=minY;y<=maxY;y++)for(let x=minX;x<=maxX;x++){water.push(...await tile(x,y,z));coverage.push(waterTileBounds(x,y,z));}
  return {coverage,water};
 }};
}
