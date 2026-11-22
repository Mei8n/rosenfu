import {expect,it,vi} from 'vitest';
import {prepareOrdinaryBusQuery,snapOrdinaryBusVertex,verifyOrdinaryBusPreview} from './ordinaryBusDraft';
import type {RoadSnapshot} from './automaticRoadData';
import {roadTileBounds,type OpenFreeMapRoadLine} from './openFreeMapRoadData';
const raw='1'.repeat(64), base='https://tiles.openfreemap.org/planet/r1/14',a={latitude:35.67504,longitude:139.748},b={latitude:35.67504,longitude:139.752};
const camera={latitude:35.67504,longitude:139.75,zoom:16};
const line:OpenFreeMapRoadLine={id:'ofm:r1:14:14552:6452:7:3',x:14552,y:6452,z:14,revision:'r1',extent:4096,class:'minor',oneway:-1,coordinates:[[a.longitude,a.latitude],[b.longitude,b.latitude]]};
const fixture=(extra:OpenFreeMapRoadLine[]=[])=>({tileSnapshots:[
 {bundle:{source:{source:base+'/14552/6452.pbf'},sourceSha256:raw},tileLines:[line,...extra.filter(l=>l.x===14552)]},
 ...(extra.some(l=>l.x===14553)?[{bundle:{source:{source:base+'/14553/6452.pbf'},sourceSha256:raw},tileLines:extra.filter(l=>l.x===14553)}]:[]),
]}) as unknown as RoadSnapshot;
const start=()=>snapOrdinaryBusVertex(fixture(),a,camera,1);
const end=()=>snapOrdinaryBusVertex(fixture(),b,camera,1,start());
it('uses actual raw feature/line/vertex, not line midpoint or a directional picker',()=>{
 const blocked={...line,id:'ofm:r1:14:14552:6452:1:3',brunnel:'bridge'};
 expect(snapOrdinaryBusVertex(fixture([blocked]),a,camera,1).raw).toEqual({feature:7,line:3,vertex:0});
 expect(end().raw).toEqual({feature:7,line:3,vertex:1});
 expect(()=>snapOrdinaryBusVertex(fixture(),camera,camera,1)).toThrow('見つけられません');
});
it('fails closed at tile boundaries instead of snapping to a different range',()=>{
 const point={latitude:35.67504,longitude:roadTileBounds(14552,6452).east+.0002};
 const next={...line,id:'ofm:r1:14:14553:6452:7:3',x:14553,coordinates:[[point.longitude,point.latitude],[point.longitude+.0007,point.latitude]] as OpenFreeMapRoadLine['coordinates']};
 expect(()=>snapOrdinaryBusVertex(fixture([next]),point,camera,1,start())).toThrow('範囲をまたぐ');
 const buffer={...line,coordinates:next.coordinates};
 expect(()=>snapOrdinaryBusVertex(fixture([buffer]),point,camera,1)).toThrow('見つけられません');
});
it('uses shared exact cache evidence, validates bindings, and does not send retained blobs again',async()=>{
 const anchor=start(),to=end();
 const key=base+'/14552/6452.pbf';
 const result={status:'ready' as const,evidence:{
  rawTiles:[{sha256:raw,bytes:Uint8Array.from([1,2,3,4])}],
  bindings:[{...anchor.tile,key,url:key,license:'ODbL-1.0' as const,attribution:'© OpenMapTiles © OpenStreetMap contributors'}]
 }};
 const read=vi.fn().mockResolvedValue(result);
 const q=await prepareOrdinaryBusQuery(anchor,to,'バス系統','bus:test','token',read,[]);
 expect(read).toHaveBeenCalledTimes(1);
 expect(q).toEqual({expectedWorldToken:'token',routeId:'bus:test',name:'バス系統',path:{
  rule:'ofm_ground_bus_z14_v1',respectOneway:false,tileKey:key,tileRawSha256:raw,start:anchor.raw,end:to.raw,
 },rawEvidence:{sha256:raw,byteLength:4,base64:'AQIDBA=='},tileBinding:result.evidence.bindings[0]});
 const prior=await prepareOrdinaryBusQuery(anchor,to,'バス系統','bus:test','token',read,[{binding:{canonical_url:key,raw_sha256:raw}}]);
 expect(prior.rawEvidence).toBeNull();expect(prior.tileBinding).toBeNull();expect(read).toHaveBeenCalledTimes(1);
 read.mockResolvedValueOnce({status:'stale'});
 await expect(prepareOrdinaryBusQuery(anchor,to,'バス系統','bus:test','token',read,[])).rejects.toThrow('更新されました');
 await expect(prepareOrdinaryBusQuery(anchor,anchor,'バス系統','bus:test','token',read,[])).rejects.toThrow('異なる道路位置');
});
it('requires native preview with physical length and exact endpoints',()=>{
 const good={points:[a,{latitude:a.latitude+.0002,longitude:a.longitude+.001},b],lengthM:410};
 expect(verifyOrdinaryBusPreview(good,start(),end())).toEqual(good);
 expect(()=>verifyOrdinaryBusPreview({points:[b,a],lengthM:2},start(),end())).toThrow();
 expect(()=>verifyOrdinaryBusPreview({points:[a,b],lengthM:NaN},start(),end())).toThrow();
});

it('requests and verifies the exact complete same-revision v2 rectangle and compactly reuses retained tiles',async()=>{
 const a0=start(),b0=end();
 const sibling=(x:number,y:number,digit:string)=>({...a0.tile,x,y,rawSha256:digit.repeat(64)});
 const refs=[a0.tile,sibling(14552,6453,'2'),sibling(14553,6452,'3'),sibling(14553,6453,'4')];
 const across={...b0,tile:refs[3]};
 const key=(t:typeof a0.tile)=>`${base}/${t.x}/${t.y}.pbf`;
 const observe=vi.fn().mockResolvedValue(refs);
 const read=vi.fn(async(toFetch:ReadonlyArray<(typeof refs)[number]>)=>({status:'ready' as const,evidence:{
  rawTiles:toFetch.map(t=>({sha256:t.rawSha256,bytes:Uint8Array.of(4)})),
  bindings:toFetch.map(t=>({...t,key:key(t),url:key(t),license:'ODbL-1.0' as const,
    attribution:'© OpenMapTiles © OpenStreetMap contributors'})),
 }}));
 const args=await prepareOrdinaryBusQuery(a0,across,'bus','B','token',read,[{binding:{canonical_url:key(a0.tile),raw_sha256:raw}}],undefined,observe);
 expect(observe).toHaveBeenCalledWith([[14552,6452],[14552,6453],[14553,6452],[14553,6453]],'r1',undefined);
 expect(read).toHaveBeenCalledWith(refs.slice(1),undefined);
 expect(args.path).toEqual({rule:'ofm_ground_bus_rectangle_z14_v2',endTileKey:key(across.tile),
  tileKey:key(a0.tile),tileRawSha256:raw,respectOneway:false,start:a0.raw,end:across.raw,
  additionalTiles:refs.slice(1).map(t=>({tileKey:key(t),tileRawSha256:t.rawSha256})),
 });
 expect(args.rawEvidence).toBeNull();expect(args.tileBinding).toBeNull();expect(args.contextTiles).toHaveLength(3);
 observe.mockResolvedValueOnce(refs.slice(1));
 await expect(prepareOrdinaryBusQuery(a0,across,'bus','B','token',read,[],undefined,observe)).rejects.toThrow('揃っていません');
 observe.mockResolvedValueOnce([...refs.slice(1),refs[1]]);
 await expect(prepareOrdinaryBusQuery(a0,across,'bus','B','token',read,[],undefined,observe)).rejects.toThrow('揃っていません');
 observe.mockResolvedValueOnce([{...a0.tile,rawSha256:'a'.repeat(64)},...refs.slice(1)]);
 await expect(prepareOrdinaryBusQuery(a0,across,'bus','B','token',read,[],undefined,observe)).rejects.toThrow('変わりました');
 const far={...across,tile:{...across.tile,x:14660}};
 await expect(prepareOrdinaryBusQuery(a0,far,'bus','B','token',read,[],undefined,observe)).rejects.toThrow('広すぎます');
 expect(read).toHaveBeenCalledTimes(1);
});

it('accepts only a matching native return or explicit reason for a requested round trip',()=>{
 const out={points:[a,b],lengthM:310};
 expect(()=>verifyOrdinaryBusPreview(out,start(),end(),'openRoundTrip')).toThrow('一致');
 expect(()=>verifyOrdinaryBusPreview({...out,returnPath:out},start(),end(),'openRoundTrip')).toThrow('一致');
 const decision={...out,returnPath:{points:[b,a],lengthM:385}};
 expect(verifyOrdinaryBusPreview(decision,start(),end(),'openRoundTrip')).toEqual(decision);
 expect(()=>verifyOrdinaryBusPreview(decision,start(),end())).toThrow('一致');
 const refused={...out,returnUnavailableReason:'復路なし'};
 expect(verifyOrdinaryBusPreview(refused,start(),end(),'openRoundTrip')).toEqual(refused);
});
