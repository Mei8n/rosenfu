import {describe,it,expect,vi} from 'vitest';
import {decodeFineWaterMask,decodeWorldWaterMask,lookupWaterModel,fetchWaterRange,type FineWaterSpec} from './waterMaskModel';
const spec:FineWaterSpec={id:'test',revision:'fixed-v1',coverage:'complete_model',originLongitude:139,originLatitude:35,x0:0,y0:0,cellM:25,width:2,height:2};
function ints(values:number[],size:2|4) {const b=new Uint8Array(values.length*size);const v=new DataView(b.buffer);values.forEach((n,i)=>size===4?v.setUint32(i*size,n,true):v.setUint16(i*size,n,true));return b;}
const lon=(m:number)=>139+m/(6371008.8*Math.cos(35*Math.PI/180))*180/Math.PI;
const lat=(m:number)=>35+m/6371008.8*180/Math.PI;
function fine(s=spec) {return decodeFineWaterMask(s,ints([0,2,2],4),ints([0,1],2));}
function world() {
 const b=new Uint8Array(16+180*3);b.set([84,87,87,49]);const v=new DataView(b.buffer);v.setUint32(4,360,true);v.setUint32(8,180,true);v.setUint32(12,1,true);
 // Water at longitude [-180,-179), every supported latitude.
 for(let row=0;row<180;row++)b.set([2,0,1],16+row*3);return b;
}
describe('water mask model',()=>{
 it('keeps fine water, fine land and uncovered separate, including half-open grid edges',()=>{
  const m=fine();expect(m.at(lon(10),lat(10))).toBe('model_water');expect(m.at(lon(40),lat(10))).toBe('model_land');
  expect(m.at(lon(10),lat(40))).toBe('model_land');expect(m.at(lon(-1),lat(10))).toBeNull();expect(m.at(lon(51),lat(10))).toBeNull();
  expect(m.at(NaN,35)).toBeNull();
 });
 it('validates rows/runs and copies the input buffers',()=>{
  for(const [rows,edges] of [[[1,2,2],[0,1]],[[0,3,2],[0,1]],[[0,2,2],[1,1]],[[0,2,2],[0,3]],[[0,4,4],[0,2,1,2]]])expect(()=>decodeFineWaterMask(spec,ints(rows,4),ints(edges,2))).toThrow();
  const r=ints([0,2,2],4),e=ints([0,1],2);const m=decodeFineWaterMask(spec,r,e);r.fill(255);e.fill(255);expect(m.at(lon(10),lat(10))).toBe('model_water');
 });
 it('reads a complete world raster and rejects truncation, excess, odd runs and invalid dimensions',()=>{
  const b=world(),m=decodeWorldWaterMask('world-v1',b);expect(m.waterAt(-179.5,35)).toBe(true);expect(m.waterAt(139,35)).toBe(false);expect(m.waterAt(180,35)).toBe(true);expect(m.waterAt(Infinity,35)).toBeNull();
  expect(()=>decodeWorldWaterMask('world-v1',b.slice(0,-1))).toThrow();expect(()=>decodeWorldWaterMask('world-v1',new Uint8Array([...b,0]))).toThrow();
  const odd=b.slice();odd[16]=1;expect(()=>decodeWorldWaterMask('world-v1',odd)).toThrow();const invalid=b.slice();invalid[12]=0;expect(()=>decodeWorldWaterMask('world-v1',invalid)).toThrow();
 });
 it('does not promote coarse land or missing fine data to model land',()=>{
  const coarse={sourceId:'coarse',waterAt:()=>true};
  expect(lookupWaterModel(lon(40),lat(10),[spec],new Map(),coarse)).toEqual({state:'unknown',reason:'fine_missing'});
  expect(lookupWaterModel(lon(40),lat(10),[spec],new Map([[spec.id,fine()]]),coarse).state).toBe('model_land');
  expect(lookupWaterModel(140,35,[spec],new Map(),coarse).state).toBe('coarse_water');
  expect(lookupWaterModel(140,35,[],new Map(),decodeWorldWaterMask('world',world()))).toEqual({state:'unknown',reason:'outside_fine'});
 });
 it('requires every overlapping fine source, reports all sources and rejects version mixing',()=>{
  const second={...spec,id:'second'};const loaded=new Map([[spec.id,fine()],[second.id,fine(second)]]);
  expect(lookupWaterModel(lon(10),lat(10),[spec,second],loaded,null)).toEqual({state:'model_water',sourceIds:['test','second'],resolutionM:25});
  loaded.delete('second');expect(lookupWaterModel(lon(10),lat(10),[spec,second],loaded,null).state).toBe('unknown');
  const next={...second,revision:'v2'};loaded.set('second',fine(next));expect(()=>lookupWaterModel(lon(10),lat(10),[spec,next],loaded,null)).toThrow('Mixed');
  expect(()=>lookupWaterModel(140,35,[spec,spec],loaded,null)).toThrow('Duplicate');
 });
});
const hash='039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81'; // SHA256 01 02 03
function response(bytes:number[],status=206,range='bytes 5-7/100') {return new Response(new Uint8Array(bytes),{status,headers:{'content-range':range}});}
describe('pinned water range acquisition',()=>{
 it('requires exact partial response and expected content hash',async()=>{
  const f=vi.fn().mockResolvedValue(response([1,2,3]));expect(await fetchWaterRange('https://example.org/water.bin',5,3,hash,new AbortController().signal,f)).toEqual(new Uint8Array([1,2,3]));
  expect(f.mock.calls[0][1].headers.Range).toBe('bytes=5-7');
  for(const r of [response([1,2,3],404),response([1,2,3],200),response([1,2,3],206,'bytes 4-6/100'),response([1,2]),response([1,2,3,4]),response([3,2,1]),response([1,2,3],206,'bytes 5-7/'+ '9'.repeat(400))]){
   await expect(fetchWaterRange('https://example.org/water.bin',5,3,hash,new AbortController().signal,vi.fn().mockResolvedValue(r))).rejects.toThrow();
  }
 });
 it('cancels a blocked body read and does not fetch an already cancelled request',async()=>{
  const c=new AbortController(),cancel=vi.fn();const stream=new ReadableStream<Uint8Array>({cancel});
  const f=vi.fn().mockResolvedValue(new Response(stream,{status:206,headers:{'content-range':'bytes 5-7/100'}}));
  const pending=fetchWaterRange('https://example.org/water.bin',5,3,hash,c.signal,f);await Promise.resolve();c.abort();await expect(pending).rejects.toThrow();expect(cancel).toHaveBeenCalled();
  const nofetch=vi.fn();await expect(fetchWaterRange('https://example.org/water.bin',5,3,hash,c.signal,nofetch)).rejects.toThrow();expect(nofetch).not.toHaveBeenCalled();
 });
});

it('never treats a null or invalid fine lookup as model land',()=>{
 for(const cell of [null,undefined,'unknown']) {
  const broken={spec,at:()=>cell} as unknown as ReturnType<typeof fine>;
  expect(lookupWaterModel(lon(10),lat(10),[spec],new Map([[spec.id,broken]]),null)).toEqual({state:'unknown',reason:'fine_missing'});
 }
});
it('checks exact half-open edges and metadata scalar types',()=>{
 const s={...spec,originLongitude:0,originLatitude:0,cellM:6371008.8*0.001*(Math.PI/180),width:1,height:1};
 const m=decodeFineWaterMask(s,ints([0,2],4),ints([0,1],2));
 expect(m.at(0,0)).toBe('model_water');expect(m.at(0.001,0)).toBeNull();expect(m.at(0,0.001)).toBeNull();expect(m.at(-0.001,0)).toBeNull();
 for(const s of [{...spec,id:5},{...spec,revision:7}])expect(()=>decodeFineWaterMask(s as unknown as FineWaterSpec,ints([0,2,2],4),ints([0,1],2))).toThrow();
 expect(()=>decodeWorldWaterMask(5 as unknown as string,world())).toThrow();
});

it('keeps observations-only empty cells unknown even with a complete coarse mask',()=>{
 const s:FineWaterSpec={...spec,coverage:'observations_only'};const m=fine(s);
 expect(m.at(lon(10),lat(10))).toBe('model_water');expect(m.at(lon(40),lat(10))).toBeNull();
 expect(lookupWaterModel(lon(40),lat(10),[s],new Map([[s.id,m]]),{sourceId:'coarse',waterAt:()=>true})).toEqual({state:'unknown',reason:'fine_incomplete'});
 expect(()=>decodeFineWaterMask({...s,coverage:undefined} as unknown as FineWaterSpec,ints([0,2,2],4),ints([0,1],2))).toThrow();
});

it('refuses externally implemented model land for an observations-only spec',()=>{
 const s:FineWaterSpec={...spec,coverage:'observations_only'};
 const forged={spec:s,at:()=> 'model_land' as const};
 expect(lookupWaterModel(lon(10),lat(10),[s],new Map([[s.id,forged]]),null)).toEqual({state:'unknown',reason:'fine_incomplete'});
});
