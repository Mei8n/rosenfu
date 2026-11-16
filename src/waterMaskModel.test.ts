import {describe,it,expect,vi} from 'vitest';
import {decodeFineWaterMask,decodeWorldWaterMask,lookupWaterModel,fetchWaterRange,fetchFineWaterMask,fetchWaterFile,fetchWorldWaterMask,lookupConstructionWater,type FineWaterSpec} from './waterMaskModel';
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

it('loads a pinned standalone observation raster without promoting empty cells',async()=>{
 const s:FineWaterSpec={...spec,coverage:'observations_only'};
 const bytes=new Uint8Array([...ints([0,2,2],4),...ints([0,1],2)]);
 const digest=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),b=>b.toString(16).padStart(2,'0')).join('');
 const f=vi.fn().mockResolvedValue(response([...bytes],200));
 const pending=fetchFineWaterMask(s,'https://example.org/water.bin',bytes.length,digest,new AbortController().signal,f);
 s.coverage='complete_model';const mask=await pending;
 expect(mask.at(lon(10),lat(10))).toBe('model_water');expect(mask.at(lon(40),lat(10))).toBeNull();
 expect(f.mock.calls[0][1].headers).toBeUndefined();
 const nofetch=vi.fn();await expect(fetchFineWaterMask(spec,'https://example.org/water.bin',3,digest,new AbortController().signal,nofetch)).rejects.toThrow();expect(nofetch).not.toHaveBeenCalled();
});

it('requires an exact complete file and does not accept partial or failed downloads',async()=>{
 for(const r of [response([1,2,3],206),response([1,2,3],404),response([1,2],200),response([1,2,3,4],200),response([3,2,1],200)]){
  await expect(fetchWaterFile('https://example.org/water.bin',3,hash,new AbortController().signal,vi.fn().mockResolvedValue(r))).rejects.toThrow();
 }
 const c=new AbortController(),cancel=vi.fn(),f=vi.fn().mockResolvedValue(new Response(new ReadableStream({cancel}),{status:200}));
 const pending=fetchWaterFile('https://example.org/water.bin',3,hash,c.signal,f);await Promise.resolve();c.abort();await expect(pending).rejects.toThrow();expect(cancel).toHaveBeenCalled();
});

it('rejects correctly hashed corrupt world files and permits a later valid retry',async()=>{
 const valid=world(),bad=valid.slice();bad[16]=1;
 const digest=async(bytes:Uint8Array<ArrayBuffer>)=>Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),b=>b.toString(16).padStart(2,'0')).join('');
 const f=vi.fn().mockResolvedValueOnce(response([...bad],200)).mockResolvedValueOnce(response([...valid],200));
 await expect(fetchWorldWaterMask('world-v1','https://example.org/world.water',bad.length,await digest(bad),new AbortController().signal,f)).rejects.toThrow('Invalid world water row');
 const coarse=await fetchWorldWaterMask('world-v1','https://example.org/world.water',valid.length,await digest(valid),new AbortController().signal,f);
 expect(f).toHaveBeenCalledTimes(2);
 expect(lookupWaterModel(-179.5,35,[],new Map(),coarse).state).toBe('coarse_water');
 expect(lookupWaterModel(139,35,[],new Map(),coarse)).toEqual({state:'unknown',reason:'outside_fine'});
 const nofetch=vi.fn();await expect(fetchWorldWaterMask('','https://example.org/world.water',valid.length,await digest(valid),new AbortController().signal,nofetch)).rejects.toThrow();expect(nofetch).not.toHaveBeenCalled();
});

describe('adopted construction map availability policy',()=>{
 const absent={state:'not_provided' as const,catalogRevision:'catalog-v1'};
 const unavailable={state:'unavailable' as const};
 const wet={state:'ready' as const,mask:{sourceId:'world-v1',waterAt:()=>true}};
 const dry={state:'ready' as const,mask:{sourceId:'world-v1',waterAt:()=>false}};
 it('uses coarse water/dry only when fine is not provided',()=>{
  expect(lookupConstructionWater(139,35,absent,wet)).toEqual({state:'model_water',basis:'coarse',sourceIds:['world-v1']});
  expect(lookupConstructionWater(139,35,absent,dry)).toEqual({state:'model_land',basis:'coarse',sourceIds:['world-v1']});
 });
 it('permits model land when both catalogs explicitly provide no model',()=>{
  expect(lookupConstructionWater(139,35,absent,absent)).toEqual({state:'model_land',basis:'not_provided',sourceIds:[]});
  expect(lookupConstructionWater(139,35,{...absent,catalogRevision:''},absent).state).toBe('unknown');
 });
 it('does not hide failed fine or coarse acquisition behind absence',()=>{
  for(const coarse of [wet,dry,absent,unavailable])expect(lookupConstructionWater(139,35,unavailable,coarse)).toEqual({state:'unknown',reason:'fine_unavailable'});
  expect(lookupConstructionWater(139,35,absent,unavailable)).toEqual({state:'unknown',reason:'coarse_unavailable'});
  expect(lookupConstructionWater(139,35,absent,{state:'ready',mask:{sourceId:'bad',waterAt:()=>null}}).state).toBe('unknown');
 });
 it('fine model land overrides coarse water; fine water needs no coarse fallback',()=>{
  const ready={state:'ready' as const,catalog:[spec],loaded:new Map([[spec.id,fine()]])};
  expect(lookupConstructionWater(lon(40),lat(10),ready,wet)).toMatchObject({state:'model_land',basis:'fine'});
  expect(lookupConstructionWater(lon(10),lat(10),ready,unavailable)).toMatchObject({state:'model_water',basis:'fine'});
  expect(lookupConstructionWater(140,35,ready,wet)).toMatchObject({state:'model_water',basis:'coarse'});
 });
 it('missing listed fine and observation gaps are not normal absence',()=>{
  expect(lookupConstructionWater(lon(10),lat(10),{state:'ready',catalog:[spec],loaded:new Map()},dry)).toEqual({state:'unknown',reason:'fine_unavailable'});
  const s:FineWaterSpec={...spec,coverage:'observations_only'};
  for(const coarse of [dry,absent])expect(lookupConstructionWater(lon(40),lat(10),{state:'ready',catalog:[s],loaded:new Map([[s.id,fine(s)]])},coarse)).toEqual({state:'unknown',reason:'fine_incomplete'});
  expect(lookupConstructionWater(NaN,35,absent,absent).state).toBe('unknown');
 });
});
