/** Independent readers for the published TW water raster formats; no TW code is included.
 * A dry fine cell is a model classification, never proof that every real stream is absent.
 * Acquisition failures and uncovered cells are distinct from model land. */
export type WaterCell = 'model_water' | 'model_land';
export type WaterLookup = {state: WaterCell; resolutionM: number; sourceIds: string[]} |
  {state: 'coarse_water'; sourceId: string} |
  {state: 'unknown'; reason: 'fine_missing' | 'fine_incomplete' | 'outside_fine' | 'invalid_coordinate'};
export interface FineWaterSpec {
  id: string; revision: string;
  coverage: 'complete_model' | 'observations_only';
  originLongitude: number; originLatitude: number;
  x0: number; y0: number; cellM: number; width: number; height: number;
}
const MAX_BYTES = 16 * 1024 * 1024;
const R = 6371008.8; // The published raster's spherical equirectangular frame.
const rad = Math.PI / 180;
function coordinate(lon: number, lat: number) {
  return Number.isFinite(lon) && Number.isFinite(lat) && Math.abs(lon) <= 180 && Math.abs(lat) <= 85;
}
function checkSpec(s: FineWaterSpec): void {
  if (typeof s.id !== 'string' || !s.id || s.id.length > 256 || typeof s.revision !== 'string' || !s.revision || s.revision.length > 256 ||
      !['complete_model','observations_only'].includes(s.coverage) ||
      !coordinate(s.originLongitude,s.originLatitude) ||
      ![s.x0,s.y0,s.cellM].every(Number.isFinite) || s.cellM <= 0 || s.cellM > 1000 ||
      !Number.isInteger(s.width) || !Number.isInteger(s.height) ||
      s.width < 1 || s.width > 65535 || s.height < 1 || s.height > 65535 || s.width*s.height > 50_000_000) {
    throw new Error('Invalid fine water grid');
  }
  const west=s.originLongitude+s.x0/(R*Math.cos(s.originLatitude*rad))/rad;
  const east=s.originLongitude+(s.x0+s.width*s.cellM)/(R*Math.cos(s.originLatitude*rad))/rad;
  const south=s.originLatitude+s.y0/R/rad;
  const north=s.originLatitude+(s.y0+s.height*s.cellM)/R/rad;
  if (!coordinate(west,south) || !coordinate(east,north)) throw new Error('Unsupported fine water extent');
}
function gridCell(s: FineWaterSpec, lon: number, lat: number): [number,number] | null {
  if (!coordinate(lon,lat)) return null;
  const x=R*(lon-s.originLongitude)*rad*Math.cos(s.originLatitude*rad);
  const y=R*(lat-s.originLatitude)*rad;
  const col=Math.floor((x-s.x0)/s.cellM), row=Math.floor((y-s.y0)/s.cellM);
  return col>=0 && row>=0 && col<s.width && row<s.height ? [col,row] : null;
}
function wet(rows: readonly number[], edges: readonly number[], col: number, row: number): boolean {
  let low=rows[row], high=rows[row+1];
  while(low<high) {const mid=(low+high)>>>1;if(edges[mid]<=col) low=mid+1;else high=mid;}
  return (low-rows[row])%2===1;
}
export interface FineWaterMask {
  readonly spec: Readonly<FineWaterSpec>;
  at(lon: number,lat: number): WaterCell | null;
}
export function decodeFineWaterMask(spec: FineWaterSpec, rowBytes: Uint8Array, runBytes: Uint8Array): FineWaterMask {
  checkSpec(spec);
  if(rowBytes.byteLength!==(spec.height+1)*4 || runBytes.byteLength%4!==0 ||
      rowBytes.byteLength+runBytes.byteLength>MAX_BYTES) throw new Error('Invalid fine water lengths');
  const rv=new DataView(rowBytes.buffer,rowBytes.byteOffset,rowBytes.byteLength);
  const ev=new DataView(runBytes.buffer,runBytes.byteOffset,runBytes.byteLength);
  const rows=Array.from({length:spec.height+1},(_,i)=>rv.getUint32(i*4,true));
  const edges=Array.from({length:runBytes.byteLength/2},(_,i)=>ev.getUint16(i*2,true));
  if(rows[0]!==0 || rows.at(-1)!==edges.length) throw new Error('Invalid fine water offsets');
  for(let r=0;r<spec.height;r++) {
    if(rows[r]%2 || rows[r+1]%2 || rows[r]>rows[r+1] || rows[r+1]>edges.length) throw new Error('Invalid fine water row');
    let end=0;
    for(let i=rows[r];i<rows[r+1];i+=2) {
      if(edges[i]<end || edges[i]>=edges[i+1] || edges[i+1]>spec.width) throw new Error('Invalid fine water run');
      end=edges[i+1];
    }
  }
  const s=Object.freeze({...spec});
  return Object.freeze({spec:s,at:(lon:number,lat:number)=>{
    const c=gridCell(s,lon,lat);return c ? wet(rows,edges,c[0],c[1])?'model_water':s.coverage==='complete_model'?'model_land':null : null;
  }});
}
export interface CoarseWaterMask { readonly sourceId: string; waterAt(lon:number,lat:number): boolean | null }
export function decodeWorldWaterMask(sourceId: string, bytes: Uint8Array): CoarseWaterMask {
  if(typeof sourceId !== 'string' || !sourceId || sourceId.length>256 || bytes.length<16 || bytes.length>MAX_BYTES ||
      String.fromCharCode(...bytes.subarray(0,4))!=='TWW1') throw new Error('Invalid world water header');
  const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);
  const width=view.getUint32(4,true),height=view.getUint32(8,true),per=view.getUint32(12,true);
  if(per<1 || per>182 || width!==360*per || height!==180*per) throw new Error('Invalid world water dimensions');
  let offset=16;
  function integer(): number {
    let n=0;
    for(let shift=0;shift<=28;shift+=7) {
      if(offset===bytes.length) throw new Error('Truncated water integer');
      const b=bytes[offset++];n+=(b&127)*2**shift;
      if(n>0xffffffff) throw new Error('Overflow water integer');
      if(!(b&128)) {if(shift>0 && b===0) throw new Error('Noncanonical water integer');return n;}
    }
    throw new Error('Oversized water integer');
  }
  const rows=[0],edges:number[]=[];
  for(let row=0;row<height;row++) {
    const n=integer();if(n%2 || n>width+1 || edges.length+n>4_000_000) throw new Error('Invalid world water row');
    let end=0;
    for(let i=0;i<n;i++) {
      const delta=integer();if(i>0 && delta===0) throw new Error('Empty world water run');
      end+=delta;if(end>width) throw new Error('World water edge out of bounds');edges.push(end);
    }
    rows.push(edges.length);
  }
  if(offset!==bytes.length) throw new Error('Trailing world water data');
  return Object.freeze({sourceId,waterAt:(lon:number,lat:number)=>{
    if(!coordinate(lon,lat))return null;
    return wet(rows,edges,Math.floor((lon===180?0:lon+180)*per),Math.floor((lat+90)*per));
  }});
}
/** The catalog is authoritative about which fine grids are required. Missing data
 * within one required grid cannot be hidden by coarse data or another loaded grid. */
export function lookupWaterModel(lon:number,lat:number,catalog:readonly FineWaterSpec[],
  loaded:ReadonlyMap<string,FineWaterMask>,coarse:CoarseWaterMask|null):WaterLookup {
  if(!coordinate(lon,lat))return {state:'unknown',reason:'invalid_coordinate'};
  if(catalog.length>1024)throw new Error('Fine water catalog limit');
  const ids=new Set<string>();const sourceIds:string[]=[];let resolutionM=0;let revision:string|null=null;let water=false;
  for(const spec of catalog) {
    checkSpec(spec);if(ids.has(spec.id))throw new Error('Duplicate fine water id');ids.add(spec.id);
    if(!gridCell(spec,lon,lat))continue;
    const mask=loaded.get(spec.id);
    if(!mask || Object.keys(spec).some(k=>spec[k as keyof FineWaterSpec]!==mask.spec[k as keyof FineWaterSpec]))return {state:'unknown',reason:'fine_missing'};
    if(revision!==null && revision!==spec.revision)throw new Error('Mixed fine water revisions');
    const cell=mask.at(lon,lat);
    if((cell!=='model_water' && cell!=='model_land') || (spec.coverage==='observations_only' && cell==='model_land'))return {state:'unknown',reason:spec.coverage==='observations_only'?'fine_incomplete':'fine_missing'};
    revision=spec.revision;sourceIds.push(spec.id);resolutionM=Math.max(resolutionM,spec.cellM);water ||= cell==='model_water';
  }
  if(sourceIds.length)return {state:water?'model_water':'model_land',resolutionM,sourceIds};
  if(coarse?.waterAt(lon,lat))return {state:'coarse_water',sourceId:coarse.sourceId};
  return {state:'unknown',reason:'outside_fine'};
}
/** Fetch only a fixed water byte range. An ignored Range, 404, truncation or hash
 * mismatch is an error, never an empty successful mask. Cancellation is caller-owned. */
export async function fetchWaterRange(url:string,from:number,length:number,sha256:string,signal:AbortSignal,
  fetcher:typeof fetch=fetch):Promise<Uint8Array> {
  if(typeof url!=='string' || !url.startsWith('https://') || !Number.isSafeInteger(from) || from<0 || !Number.isSafeInteger(length) ||
      length<1 || length>MAX_BYTES || !Number.isSafeInteger(from+length) || typeof sha256!=='string' || !/^[0-9a-f]{64}$/.test(sha256)) throw new Error('Invalid water range descriptor');
  signal.throwIfAborted();
  const end=from+length-1;
  const response=await fetcher(url,{headers:{Range:`bytes=${from}-${end}`},signal});
  const range=response.headers.get('content-range');
  const match=range?.match(/^bytes (\d+)-(\d+)\/(\d+)$/);
  if(response.status!==206 || !match || Number(match[1])!==from || Number(match[2])!==end || !Number.isSafeInteger(Number(match[3])) || Number(match[3])<=end || !response.body) {
    await response.body?.cancel();throw new Error('Water range unavailable');
  }
  return readPinnedBody(response,length,sha256,signal);
}
async function readPinnedBody(response: Response,length: number,sha256: string,signal: AbortSignal): Promise<Uint8Array> {
  if(!response.body)throw new Error('Missing water body');
  const reader=response.body.getReader();const bytes=new Uint8Array(length);let written=0;
  const abort=()=>{void reader.cancel().catch(()=>{});};
  signal.addEventListener('abort',abort,{once:true});
  try {
    for(;;) {
      signal.throwIfAborted();const part=await reader.read();signal.throwIfAborted();if(part.done)break;
      if(part.value.length>length-written)throw new Error('Water range exceeds limit');
      bytes.set(part.value,written);written+=part.value.length;
    }
    if(written!==length)throw new Error('Truncated water range');
  } catch(error) {await reader.cancel().catch(()=>{});throw error;} finally {signal.removeEventListener('abort',abort);reader.releaseLock();}
  const digest=await crypto.subtle.digest('SHA-256',bytes);signal.throwIfAborted();
  const actual=Array.from(new Uint8Array(digest),b=>b.toString(16).padStart(2,'0')).join('');
  if(actual!==sha256)throw new Error('Water range hash mismatch');
  return bytes;
}

/** Load a separately published, pinned fine raster. Snapshot the declared spec
 * before awaiting transport so a caller cannot change coverage during a fetch. */
export async function fetchFineWaterMask(spec: FineWaterSpec, url: string, length: number,
  sha256: string, signal: AbortSignal, fetcher: typeof fetch = fetch): Promise<FineWaterMask> {
  const fixed = {...spec};
  checkSpec(fixed);
  const rowBytes = (fixed.height + 1) * 4;
  if (!Number.isSafeInteger(length) || length < rowBytes || (length-rowBytes)%4 !== 0) {
    throw new Error('Invalid fine raster descriptor');
  }
  const bytes = await fetchWaterFile(url, length, sha256, signal, fetcher);
  return decodeFineWaterMask(fixed, bytes.subarray(0,rowBytes), bytes.subarray(rowBytes));
}

/** A small standalone file, not a fallback for ignored byte ranges. This avoids
 * relying on Content-Range exposure in CORS while still verifying every byte. */
export async function fetchWaterFile(url:string,length:number,sha256:string,signal:AbortSignal,
  fetcher:typeof fetch=fetch):Promise<Uint8Array> {
  if(typeof url!=='string' || !url.startsWith('https://') || !Number.isSafeInteger(length) ||
      length<1 || length>MAX_BYTES || typeof sha256!=='string' || !/^[0-9a-f]{64}$/.test(sha256)) throw new Error('Invalid water file descriptor');
  signal.throwIfAborted();
  const response=await fetcher(url,{signal});
  if(response.status!==200 || !response.body) {
    await response.body?.cancel();throw new Error('Water file unavailable');
  }
  return readPinnedBody(response,length,sha256,signal);
}
