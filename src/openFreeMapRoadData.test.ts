import {describe,it,expect,vi} from 'vitest';
import {PbfWriter} from 'pbf';
import {snapOrdinaryBusVertex} from './ordinaryBusDraft';
import {decodeOpenFreeMapRoadTile,parseRoadTileTemplate,roadTileBounds,createOpenFreeMapRoadRepository,roadFragmentIntersects,OFM_EVIDENCE_RULE} from './openFreeMapRoadData';
function fixture(cls='service',brunnel?:string,subclass?:string,extra:Record<string,string|number|boolean>={}){const attrs={class:cls,...(brunnel?{brunnel}:{}),...(subclass?{subclass}:{}),...extra};const p=new PbfWriter();p.writeMessage(3,(_,l)=>{l.writeStringField(1,'transportation');l.writeVarintField(15,2);l.writeVarintField(5,4096);for(const [k,v]of Object.entries(attrs)){l.writeStringField(3,k);l.writeMessage(4,(_,w)=>typeof v==='number'?w.writeDoubleField(3,v):typeof v==='boolean'?w.writeBooleanField(7,v):w.writeStringField(1,v),null);}l.writeMessage(2,(_,f)=>{f.writeVarintField(1,99);f.writePackedVarint(2,Object.keys(attrs).flatMap((_,i)=>[i,i]));f.writeVarintField(3,2);f.writePackedVarint(4,[9,200,200,10,200,0]);},null);},null);return p.finish();}
function mixedFordTile(){const p=new PbfWriter();p.writeMessage(3,(_,l)=>{l.writeStringField(1,'transportation');l.writeVarintField(15,2);l.writeVarintField(5,4096);
  for(const k of ['class','brunnel'])l.writeStringField(3,k);
  for(const v of ['minor','ford'])l.writeMessage(4,(_,p)=>p.writeStringField(1,v),null);
  for(const [attrs,x] of [[[0,0,1,1],200],[[0,0],1600]] as const)l.writeMessage(2,(_,f)=>{f.writePackedVarint(2,[...attrs]);f.writeVarintField(3,2);f.writePackedVarint(4,[9,x,200,10,200,0]);},null);
 },null);return p.finish();}
function denseFixture(east:boolean){const p=new PbfWriter();p.writeMessage(3,(_,l)=>{l.writeStringField(1,'transportation');l.writeVarintField(15,2);l.writeVarintField(5,4096);l.writeStringField(3,'class');l.writeMessage(4,(_,v)=>v.writeStringField(1,'service'),null);for(let i=0;i<6000;i++)l.writeMessage(2,(_,f)=>{f.writePackedVarint(2,[0,0]);f.writeVarintField(3,2);f.writePackedVarint(4,i===5999?[9,(east?0:4090)*2,4000,10,40,0]:[9,200,200,10,200,0]);},null);},null);return p.finish();}

const decode=(b:Uint8Array)=>decodeOpenFreeMapRoadTile(b,14552,6451,'20261101_080013_pt',1);
describe('OpenFreeMap road adapter',()=>{
 it('keeps service unlike TW crossing filter and uses honest tile provenance',async()=>{const s=await decode(fixture());expect(s.roads).toHaveLength(1);expect(s.bundle.graph.nodes[0].osmId).toMatch(/^ofm:/);expect(s.bundle.roadWays![s.roads[0].id].highway).toBe('service');expect(s.coverageVerified).toBe(false);expect(s.bundle.source.sourceDate).toBeNull();});
 it('preserves bridges and tunnels separately',async()=>{expect((await decode(fixture('minor','bridge'))).roads[0].bridge).toBe('yes');expect((await decode(fixture('service','tunnel'))).roads[0].tunnel).toBe('yes');});
 it('keeps structural roads as observations but does not permit ground following',async()=>{for(const kind of ['bridge','tunnel']){const s=await decode(fixture('service',kind));expect(s.bundle.roadWays![s.roads[0].id].structureUnsupported).toBe(true);}});
 it('excludes rail but preserves paths as non-motor passage observations',async()=>{expect((await decode(fixture('rail'))).roads).toHaveLength(0);const p=await decode(fixture('path'));expect(p.roads).toHaveLength(1);expect(p.tileLines[0].class).toBe('path');expect(p.bundle.roadWays![p.roads[0].id].structureUnsupported).toBe(true);});
 it('does not turn absent layer or malformed data into empty road coverage',async()=>{await expect(decode(new Uint8Array())).rejects.toThrow('未取得');await expect(decode(new Uint8Array([255]))).rejects.toThrow();});
 it('keeps steps classified separately from vehicle roads',async()=>{const s=await decode(fixture('path',undefined,'steps'));expect(s.roads[0]).toHaveProperty('classification',{class:'path',subclass:'steps',access:null,oneway:null});expect(s.tileLines[0].subclass).toBe('steps');});
 it('rejects nonfinite structures before exposing raw tile lines',async()=>{for(const attrs of ([{layer:'abc'},{layer:Infinity},{oneway:NaN},{oneway:Infinity}] as Record<string,string|number>[]))await expect(decode(fixture('service',undefined,undefined,attrs))).rejects.toThrow();});
 it('preserves raw one-way direction without inventing missing tags',async()=>{const s=await decode(fixture('service',undefined,undefined,{oneway:-1}));expect(s.tileLines[0].oneway).toBe(-1);expect(s.roads[0]).toHaveProperty('classification.oneway','-1');const unknown=await decode(fixture('service',undefined,undefined,{oneway:'reversible'}));expect(unknown.tileLines[0].oneway).toBe('reversible');});
 it('selects crossing and boundary-touch fragments without changing shape',()=>{const b={west:0,east:1,south:0,north:1};const p=(x:number,y:number)=>({longitude:x,latitude:y});expect(roadFragmentIntersects([p(-1,.5),p(2,.5)],b)).toBe(true);expect(roadFragmentIntersects([p(-1,0),p(2,0)],b)).toBe(true);expect(roadFragmentIntersects([p(-1,-.1),p(2,-.1)],b)).toBe(false);expect(roadFragmentIntersects([p(2,2),p(3,3)],b)).toBe(false);});
 it('keeps a ford observable without losing ordinary roads in its tile or offering the ford as a bus anchor',async()=>{
  const raw=mixedFordTile(),revision='20261101_080013_pt';
  const fetcher=vi.fn(async(url:unknown)=>new Response(String(url).endsWith('/planet')?
   JSON.stringify({maxzoom:14,tiles:[`https://tiles.openfreemap.org/planet/${revision}/{z}/{x}/{y}.pbf`]}):new Uint8Array(raw)));
  const repo=createOpenFreeMapRoadRepository(fetcher as typeof fetch),b=roadTileBounds(14552,6451);
  const s=await repo.load({west:b.west+.00005,east:b.east-.00005,south:b.south+.00005,north:b.north-.00005});
  const [ford,ordinary]=s.tileLines;
  expect([ford.brunnel,ordinary.brunnel]).toEqual(['ford',undefined]);
  const snapshot=s.tileSnapshots[0],fordObservation=snapshot.roads.find(road=>road.id===ford.id)!;
  expect(fordObservation).toMatchObject({bridge:'ford',tunnel:null});
  expect(snapshot.bundle.roadWays![ford.id].structureUnsupported).toBe(true);
  expect(snapshot.bundle.graph.edges.find(edge=>edge.osmWayId===ford.id)).toMatchObject({bridge:null,tunnel:null});
  const asPoint=([longitude,latitude]:[number,number])=>({longitude,latitude});
  const camera={latitude:35.68,longitude:139.76,zoom:20,width:900,height:600};
  expect(snapOrdinaryBusVertex(s,asPoint(ordinary.coordinates[0]),camera,1).raw.feature).toBe(1);
  expect(()=>snapOrdinaryBusVertex(s,asPoint(ford.coordinates[0]),camera,1)).toThrow('道路上の点');
  const evidence=await repo.readTileEvidenceBatch([{revision,z:14,x:14552,y:6451,rawSha256:snapshot.bundle.sourceSha256,decoderRule:OFM_EVIDENCE_RULE}]);
  expect(evidence.rawTiles[0].bytes).toEqual(raw);
  expect(fetcher).toHaveBeenCalledTimes(2);
 });
 it('rejects unknown brunnel instead of persisting ground-like null flags',async()=>{for(const brunnel of ['unknown','yes','no'])await expect(decode(fixture('service',brunnel))).rejects.toThrow('地上とは判定できません');});
 it('rejects present invalid tag types rather than treating them as missing',async()=>{for(const key of ['brunnel','layer','access','oneway','subclass'])await expect(decode(fixture('service',undefined,undefined,{[key]:true}))).rejects.toThrow('型が不正');await expect(decode(fixture('service',undefined,undefined,{brunnel:0}))).rejects.toThrow('型が不正');});
 it('rejects arbitrary provider URLs',()=>{expect(()=>parseRoadTileTemplate({tiles:['https://evil.example/{z}/{x}/{y}.pbf'],maxzoom:14})).toThrow();});
 it('loads/coalesces a tile and retains per-tile source identity',async()=>{const fetcher=vi.fn(async(url:unknown)=>new Response(String(url).endsWith('/planet')?JSON.stringify({tiles:['https://tiles.openfreemap.org/planet/20261101_080013_pt/{z}/{x}/{y}.pbf'],maxzoom:14}):new Uint8Array(fixture()),{status:200}));const r=createOpenFreeMapRoadRepository(fetcher as typeof fetch);const b=roadTileBounds(14552,6451);const q={west:b.west+.001,east:b.east-.001,south:b.south+.001,north:b.north-.001};const [a,c]=await Promise.all([r.load(q),r.load(q)]);expect(a.tileSnapshots).toHaveLength(1);expect(c.bundle.sourceSha256).toBe(a.bundle.sourceSha256);expect(fetcher).toHaveBeenCalledTimes(2);});
 it('limits only aggregate follow graph, preserving full two-tile observations',async()=>{const west=denseFixture(false),east=denseFixture(true);const f=vi.fn(async(url:unknown)=>new Response(String(url).endsWith('/planet')?JSON.stringify({maxzoom:14,tiles:['https://tiles.openfreemap.org/planet/rev/{z}/{x}/{y}.pbf']}):new Uint8Array(String(url).includes('/14552/')?west:east)));const s=await createOpenFreeMapRoadRepository(f as typeof fetch).load({west:139.76706640625,east:139.76906640625,south:35.683,north:35.685});expect(s.tileSnapshots.reduce((n,t)=>n+t.bundle.graph.nodes.length,0)).toBe(24000);expect(s.roads).toHaveLength(12000);expect(s.bundle.graph.nodes).toHaveLength(4);expect(s.bundle.graph.edges).toHaveLength(2);});
 it('failure stays unknown, cooldown prevents hammering',async()=>{const fetcher=vi.fn(async(url:unknown)=>new Response(String(url).endsWith('/planet')?JSON.stringify({tiles:['https://tiles.openfreemap.org/planet/rev/{z}/{x}/{y}.pbf'],maxzoom:14}):'',{status:String(url).endsWith('/planet')?200:404}));const r=createOpenFreeMapRoadRepository(fetcher as typeof fetch,()=>1);const b={west:139.75,east:139.751,south:35.68,north:35.681};await expect(r.load(b)).rejects.toThrow('404');await expect(r.load(b)).rejects.toThrow('再試行');expect(fetcher).toHaveBeenCalledTimes(2);});
 it('cancels one subscriber without stopping another shared tile',async()=>{
  let complete:(r:Response)=>void=()=>{};let tileSignal:AbortSignal|undefined;
  const f=vi.fn(async(url:unknown,init?:RequestInit)=>{if(String(url).endsWith('/planet'))return new Response(JSON.stringify({maxzoom:14,tiles:['https://tiles.openfreemap.org/planet/rev/{z}/{x}/{y}.pbf']}));tileSignal=init?.signal as AbortSignal;return new Promise<Response>((resolve,reject)=>{complete=resolve;tileSignal?.addEventListener('abort',()=>reject(new DOMException('cancel','AbortError')));});});
  const repo=createOpenFreeMapRoadRepository(f as typeof fetch),a=new AbortController(),b={west:139.75,east:139.751,south:35.68,north:35.681};
  const first=repo.load(b,a.signal),second=repo.load(b);const rejected=expect(first).rejects.toMatchObject({name:'AbortError',message:'道路取得を取り消しました。'});
  await vi.waitFor(()=>expect(f).toHaveBeenCalledTimes(2));a.abort();await rejected;expect(tileSignal?.aborted).toBe(false);complete(new Response(new Uint8Array(fixture())));expect((await second).roads).toHaveLength(1);
 });
 it('aborts last subscriber and retries without a cancellation cooldown',async()=>{
  let n=0;let active:AbortSignal|undefined;
  const f=vi.fn(async(url:unknown,init?:RequestInit)=>{if(String(url).endsWith('/planet'))return new Response(JSON.stringify({maxzoom:14,tiles:['https://tiles.openfreemap.org/planet/rev/{z}/{x}/{y}.pbf']}));if(++n>1)return new Response(new Uint8Array(fixture()));active=init?.signal as AbortSignal;return new Promise<Response>((_,reject)=>active?.addEventListener('abort',()=>reject(new DOMException('cancel','AbortError'))));});
  const repo=createOpenFreeMapRoadRepository(f as typeof fetch),a=new AbortController(),b={west:139.75,east:139.751,south:35.68,north:35.681};const first=repo.load(b,a.signal),rejected=expect(first).rejects.toMatchObject({name:'AbortError'});await vi.waitFor(()=>expect(f).toHaveBeenCalledTimes(2));a.abort();await rejected;expect(active?.aborted).toBe(true);expect((await repo.load(b)).roads).toHaveLength(1);
 });
 it('expires the whole load and skips its queued tiles',async()=>{
  vi.useFakeTimers();try{
   let tileSignal:AbortSignal|undefined;
   const f=vi.fn(async(url:unknown,init?:RequestInit)=>{if(String(url).endsWith('/planet')){await new Promise(r=>setTimeout(r,19000));return new Response(JSON.stringify({maxzoom:14,tiles:['https://tiles.openfreemap.org/planet/rev/{z}/{x}/{y}.pbf']}));}tileSignal=init?.signal as AbortSignal;return new Promise<Response>((_,reject)=>tileSignal?.addEventListener('abort',()=>reject(new DOMException('cancel','AbortError'))));});
   const repo=createOpenFreeMapRoadRepository(f as typeof fetch);const p=repo.load({west:139.767,east:139.769,south:35.683,north:35.685});const rejected=expect(p).rejects.toMatchObject({name:'TimeoutError',message:'道路取得が25秒で完了しませんでした。'});await vi.advanceTimersByTimeAsync(25001);await rejected;await vi.advanceTimersByTimeAsync(50000);expect(f).toHaveBeenCalledTimes(2);expect(tileSignal?.aborted).toBe(true);
  }finally{vi.useRealTimers();}
 });

 it('reports a network timeout separately from manual cancellation',async()=>{vi.useFakeTimers();try{const f=vi.fn(async(url:unknown,init?:RequestInit)=>{if(String(url).endsWith('/planet'))return new Response(JSON.stringify({maxzoom:14,tiles:['https://tiles.openfreemap.org/planet/rev/{z}/{x}/{y}.pbf']}));return new Promise<Response>((_,reject)=>init?.signal?.addEventListener('abort',()=>reject(new DOMException('aborted','AbortError'))));});const p=createOpenFreeMapRoadRepository(f as typeof fetch).load({west:139.75,east:139.751,south:35.68,north:35.681});const rejected=expect(p).rejects.toMatchObject({name:'TimeoutError',message:'道路データの通信が20秒で完了しませんでした。'});await vi.advanceTimersByTimeAsync(20001);await rejected;}finally{vi.useRealTimers();}});
 it('does no fetch for an already cancelled consumer',async()=>{const f=vi.fn();const c=new AbortController();c.abort();await expect(createOpenFreeMapRoadRepository(f).load({west:139.75,east:139.751,south:35.68,north:35.681},c.signal)).rejects.toMatchObject({name:'AbortError'});expect(f).not.toHaveBeenCalled();});
 it('cancels metadata with no consumers and allows immediate retry',async()=>{let n=0;let signal:AbortSignal|undefined;const f=vi.fn(async(url:unknown,init?:RequestInit)=>{if(++n===1){signal=init?.signal as AbortSignal;return new Promise<Response>((_,reject)=>signal?.addEventListener('abort',()=>reject(new DOMException('cancel','AbortError'))));}return new Response(String(url).endsWith('/planet')?JSON.stringify({maxzoom:14,tiles:['https://tiles.openfreemap.org/planet/rev/{z}/{x}/{y}.pbf']}):new Uint8Array(fixture()));});const r=createOpenFreeMapRoadRepository(f as typeof fetch),c=new AbortController(),b={west:139.75,east:139.751,south:35.68,north:35.681};const p=r.load(b,c.signal),rejected=expect(p).rejects.toMatchObject({name:'AbortError'});await vi.waitFor(()=>expect(f).toHaveBeenCalledTimes(1));c.abort();await rejected;expect(signal?.aborted).toBe(true);expect((await r.load(b)).roads).toHaveLength(1);});

 it('returns exact immutable raw evidence with canonical bindings and no duplicate fetch',async()=>{const raw=fixture();const f=vi.fn(async(url:unknown)=>new Response(String(url).endsWith('/planet')?JSON.stringify({maxzoom:14,tiles:['https://tiles.openfreemap.org/planet/rev/{z}/{x}/{y}.pbf']}):new Uint8Array(raw)));const r=createOpenFreeMapRoadRepository(f as typeof fetch);const s=await r.load({west:139.75,east:139.751,south:35.68,north:35.681});const ref={revision:'rev',z:14 as const,x:14552,y:6451,rawSha256:s.tileSnapshots[0].bundle.sourceSha256,decoderRule:OFM_EVIDENCE_RULE};const e=await r.readTileEvidenceBatch([ref]);expect(e.rawTiles[0].bytes).toEqual(raw);expect(e.bindings[0].url).toBe('https://tiles.openfreemap.org/planet/rev/14/14552/6451.pbf');expect(e.bindings[0].key).toBe(e.bindings[0].url);e.rawTiles[0].bytes.fill(0);expect((await r.readTileEvidenceBatch([ref])).rawTiles[0].bytes).toEqual(raw);expect(f).toHaveBeenCalledTimes(2);await expect(r.readTileEvidenceBatch([{...ref,rawSha256:'0'.repeat(64)}])).rejects.toThrow('一致');await expect(r.readTileEvidenceBatch(Array(17).fill(ref))).rejects.toThrow('上限');expect(f).toHaveBeenCalledTimes(2);});
 it('refetches only evicted pinned bytes and rejects changed source content',async()=>{let changed=false;const raw=fixture();const f=vi.fn(async(url:unknown)=>new Response(String(url).endsWith('/planet')?JSON.stringify({maxzoom:14,tiles:['https://tiles.openfreemap.org/planet/rev/{z}/{x}/{y}.pbf']}):new Uint8Array(changed?fixture('primary'):raw)));const r=createOpenFreeMapRoadRepository(f as typeof fetch);let sha='';for(let x=14530;x<14547;x++){const b=roadTileBounds(x,6451);const s=await r.load({west:b.west+.001,east:b.east-.001,south:b.south+.001,north:b.north-.001});sha=s.tileSnapshots[0].bundle.sourceSha256;}const ref={revision:'rev',z:14 as const,x:14530,y:6451,rawSha256:sha,decoderRule:OFM_EVIDENCE_RULE};changed=true;await expect(r.readTileEvidenceBatch([ref])).rejects.toThrow('SHA');changed=false;expect((await r.readTileEvidenceBatch([ref])).rawTiles[0].bytes).toEqual(raw);expect(f).toHaveBeenCalledTimes(20);});
 it('does not fetch raw evidence for an unobserved tile',async()=>{const f=vi.fn();const r=createOpenFreeMapRoadRepository(f);await expect(r.readTileEvidenceBatch([{revision:'rev',z:14,x:0,y:0,rawSha256:'0'.repeat(64),decoderRule:OFM_EVIDENCE_RULE}])).rejects.toThrow('取得済み');expect(f).not.toHaveBeenCalled();});

 it('enforces the Rust-compatible 8MiB budget even for reused raw bindings',async()=>{const large=(value:number)=>{const p=new PbfWriter();p.writeBytesField(99,new Uint8Array(4500000).fill(value));const suffix=p.finish(),base=fixture(),b=new Uint8Array(base.length+suffix.length);b.set(base);b.set(suffix,base.length);return b;};const a=large(1),b=large(1);const f=vi.fn(async(url:unknown)=>new Response(String(url).endsWith('/planet')?JSON.stringify({maxzoom:14,tiles:['https://tiles.openfreemap.org/planet/rev/{z}/{x}/{y}.pbf']}):new Uint8Array(String(url).includes('/14552/')?a:b)));const r=createOpenFreeMapRoadRepository(f as typeof fetch);const s=await r.load({west:139.767,east:139.769,south:35.683,north:35.685});await expect(r.readTileEvidenceBatch(s.tileSnapshots.map((t,i)=>({revision:'rev',z:14 as const,x:14552+i,y:6451,rawSha256:t.bundle.sourceSha256,decoderRule:OFM_EVIDENCE_RULE})))).rejects.toThrow('8MiB');});

 it('rejects already-aborted evidence even on an observed raw cache hit',async()=>{
  const f=vi.fn(async(url:unknown)=>new Response(String(url).endsWith('/planet')?JSON.stringify({maxzoom:14,tiles:['https://tiles.openfreemap.org/planet/rev/{z}/{x}/{y}.pbf']}):new Uint8Array(fixture())));
  const r=createOpenFreeMapRoadRepository(f as typeof fetch),s=await r.load({west:139.75,east:139.751,south:35.68,north:35.681});const c=new AbortController();c.abort();
  await expect(r.readTileEvidenceBatch([{revision:'rev',z:14,x:14552,y:6451,rawSha256:s.tileSnapshots[0].bundle.sourceSha256,decoderRule:OFM_EVIDENCE_RULE}],c.signal)).rejects.toMatchObject({name:'AbortError'});expect(f).toHaveBeenCalledTimes(2);
 });
 it('evicts by raw byte budget and refreshes LRU on evidence cache hits',async()=>{
  const p=new PbfWriter();p.writeBytesField(99,new Uint8Array(4500000));const base=fixture(),tail=p.finish(),raw=new Uint8Array(base.length+tail.length);raw.set(base);raw.set(tail,base.length);
  const f=vi.fn(async(url:unknown)=>new Response(String(url).endsWith('/planet')?JSON.stringify({maxzoom:14,tiles:['https://tiles.openfreemap.org/planet/rev/{z}/{x}/{y}.pbf']}):new Uint8Array(raw)));
  const r=createOpenFreeMapRoadRepository(f as typeof fetch);const refs=[];
  for(let x=14530;x<14533;x++){const b=roadTileBounds(x,6451),s=await r.load({west:b.west+.001,east:b.east-.001,south:b.south+.001,north:b.north-.001});refs.push({revision:'rev',z:14 as const,x,y:6451,rawSha256:s.tileSnapshots[0].bundle.sourceSha256,decoderRule:OFM_EVIDENCE_RULE});}
  await r.readTileEvidenceBatch([refs[0]]);const b=roadTileBounds(14533,6451);await r.load({west:b.west+.001,east:b.east-.001,south:b.south+.001,north:b.north-.001});expect(f).toHaveBeenCalledTimes(5);
  await r.readTileEvidenceBatch([refs[0]]);expect(f).toHaveBeenCalledTimes(5);await r.readTileEvidenceBatch([refs[1]]);expect(f).toHaveBeenCalledTimes(6);
 });
 it.each(['cancel','active cancel','deadline'] as const)('stops %s evidence without cancelling unrelated work',async(kind)=>{
  let slow=false;const f=vi.fn(async(url:unknown,init?:RequestInit)=>{if(String(url).endsWith('/planet'))return new Response(JSON.stringify({maxzoom:14,tiles:['https://tiles.openfreemap.org/planet/rev/{z}/{x}/{y}.pbf']}));if(slow){await new Promise<void>((resolve,reject)=>{const t=setTimeout(resolve,19000);init?.signal?.addEventListener('abort',()=>{clearTimeout(t);reject(new DOMException('aborted','AbortError'));});});}return new Response(new Uint8Array(fixture()));});
  const r=createOpenFreeMapRoadRepository(f as typeof fetch);let sha='';for(let x=14530;x<14547;x++){const b=roadTileBounds(x,6451),s=await r.load({west:b.west+.001,east:b.east-.001,south:b.south+.001,north:b.north-.001});sha=s.tileSnapshots[0].bundle.sourceSha256;}
  vi.useFakeTimers();try{slow=true;const b=roadTileBounds(14550,6451);const busy=r.load({west:b.west+.001,east:b.east-.001,south:b.south+.001,north:b.north-.001});await vi.advanceTimersByTimeAsync(1);const c=new AbortController();
   const pending=r.readTileEvidenceBatch([{revision:'rev',z:14,x:14530,y:6451,rawSha256:sha,decoderRule:OFM_EVIDENCE_RULE}],c.signal);const rejected=expect(pending).rejects.toMatchObject({name:kind==='deadline'?'TimeoutError':'AbortError'});
   if(kind==='cancel')c.abort();await vi.advanceTimersByTimeAsync(19000);await busy;await vi.advanceTimersByTimeAsync(1);if(kind==='active cancel')c.abort();await vi.advanceTimersByTimeAsync(6001);await rejected;await vi.advanceTimersByTimeAsync(30000);
   // Cancelled queued evidence never fetches; deadline evidence starts after the unrelated 19s request, then aborts at its own 25s lifetime.
   expect(f).toHaveBeenCalledTimes(kind==='cancel'?19:20);
  }finally{vi.useRealTimers();}
 });

});

it('observes a rectangle through the same raw provider without re-downloading its viewport cache cells',async()=>{
 const raw=fixture(), f=vi.fn(async(url:unknown)=>new Response(String(url).endsWith('/planet')?
  JSON.stringify({maxzoom:14,tiles:['https://tiles.openfreemap.org/planet/rev/{z}/{x}/{y}.pbf']}):new Uint8Array(raw)));
 const repo=createOpenFreeMapRoadRepository(f as typeof fetch);
 await repo.load({west:139.75,east:139.751,south:35.683,north:35.685});
 const cells=[[14552,6451],[14552,6452],[14553,6451],[14553,6452]] as const;
 const refs=await repo.observeTiles(cells,'rev');
 expect(refs.map(r=>[r.x,r.y])).toEqual(cells);
 const evidence=await repo.readTileEvidenceBatch(refs);
 expect(evidence.bindings).toHaveLength(4); expect(evidence.rawTiles).toHaveLength(1);
 expect(f).toHaveBeenCalledTimes(5); // metadata plus four unique raw tiles
 await expect(repo.observeTiles(cells,'stale')).rejects.toThrow('版');
 await expect(repo.observeTiles([cells[0],cells[0]],'rev')).rejects.toThrow('範囲');
 expect(f).toHaveBeenCalledTimes(5);
});

it('requests the exact catalog revision despite live rollover, without admitting fallback bytes',async()=>{
 const current='20261122_080013_pt',old='20261101_080013_pt';
 const oldRaw=fixture('service'),latestRaw=fixture('primary');
 const oldSha=(await decode(oldRaw)).bundle.sourceSha256;
 let reply:'fallback'|'missing'|'exact'='fallback';
 const f=vi.fn(async(url:unknown,_init?:RequestInit)=>{
  if(String(url).endsWith('/planet'))return new Response(JSON.stringify({maxzoom:14,tiles:[`https://tiles.openfreemap.org/planet/${current}/{z}/{x}/{y}.pbf`]}));
  if(String(url).includes('/'+old+'/'))return reply==='missing'?new Response(null,{status:404}):new Response(new Uint8Array(reply==='exact'?oldRaw:latestRaw));
  return new Response(new Uint8Array(latestRaw));
 });
 const repository=createOpenFreeMapRoadRepository(f as typeof fetch);
 await repository.load({west:139.75,east:139.751,south:35.68,north:35.681});
 const pin={revision:old,z:14 as const,x:14552,y:6451,rawSha256:oldSha,decoderRule:OFM_EVIDENCE_RULE};
 await expect(repository.readPinnedSeamTiles([pin])).rejects.toThrow('SHA-256');
 reply='missing';await expect(repository.readPinnedSeamTiles([pin])).rejects.toThrow('404');
 reply='exact';const exact=await repository.readPinnedSeamTiles([pin]);
 expect(exact.rawTiles[0].bytes).toEqual(oldRaw);
 expect(f).toHaveBeenLastCalledWith(`https://tiles.openfreemap.org/planet/${old}/14/14552/6451.pbf`,expect.objectContaining({redirect:'error'}));
 await expect(repository.readTileEvidenceBatch([pin])).rejects.toThrow('取得済み');
 expect(f.mock.calls.filter(([url])=>String(url).endsWith('/planet'))).toHaveLength(1);
});

const interiorOfTile=(x:number,y=6451)=>{
 const b=roadTileBounds(x,y);
 return {west:b.west+.001,east:b.east-.001,south:b.south+.001,north:b.north-.001};
};

it('keeps a revisited tile decoded across one full cache scan without redownloading or rebuilding it',async()=>{
 const raw=fixture(), f=vi.fn(async(url:unknown)=>new Response(String(url).endsWith('/planet')?
  JSON.stringify({maxzoom:14,tiles:['https://tiles.openfreemap.org/planet/rev/{z}/{x}/{y}.pbf']}):new Uint8Array(raw)));
 const repo=createOpenFreeMapRoadRepository(f as typeof fetch),hot=14500,cold=14501;
 const first=await repo.load(interiorOfTile(hot));
 for(let x=cold;x<hot+64;x++)await repo.load(interiorOfTile(x));
 expect(f).toHaveBeenCalledTimes(65);
 const justUsed=await repo.load(interiorOfTile(hot));
 expect(justUsed.tileSnapshots[0]).toBe(first.tileSnapshots[0]);
 await repo.load(interiorOfTile(hot+64));
 expect(f).toHaveBeenCalledTimes(66);
 const again=await repo.load(interiorOfTile(hot));
 expect(again.tileSnapshots[0]).toBe(first.tileSnapshots[0]);
 expect(again.bundle.graphSha256).toBe(first.bundle.graphSha256);
 expect(f).toHaveBeenCalledTimes(66);
 await repo.load(interiorOfTile(cold));
 expect(f).toHaveBeenCalledTimes(67);
});

it('keeps revisited raw bytes hot for exact evidence without changing decoded or raw identities',async()=>{
 const raw=fixture(),f=vi.fn(async(url:unknown)=>new Response(String(url).endsWith('/planet')?
  JSON.stringify({maxzoom:14,tiles:['https://tiles.openfreemap.org/planet/rev/{z}/{x}/{y}.pbf']}):new Uint8Array(raw)));
 const repo=createOpenFreeMapRoadRepository(f as typeof fetch),hot=14500,cold=14501;
 const first=await repo.load(interiorOfTile(hot));
 const rawSha256=first.tileSnapshots[0].bundle.sourceSha256;
 for(let x=cold;x<hot+16;x++)await repo.load(interiorOfTile(x));
 await repo.load(interiorOfTile(hot));
 await repo.load(interiorOfTile(hot+16));
 expect(f).toHaveBeenCalledTimes(18);
 const ref={revision:'rev',z:14 as const,x:hot,y:6451,rawSha256,decoderRule:OFM_EVIDENCE_RULE};
 await expect(repo.readTileEvidenceBatch([ref])).resolves.toMatchObject({rawTiles:[{sha256:rawSha256,bytes:raw}]});
 expect(f).toHaveBeenCalledTimes(18);
 await expect(repo.readTileEvidenceBatch([{...ref,rawSha256:'0'.repeat(64)}])).rejects.toThrow('一致');
 expect(f).toHaveBeenCalledTimes(18);
 await repo.readTileEvidenceBatch([{...ref,x:cold}]);
 expect(f).toHaveBeenCalledTimes(19);
});

it('retains provider backoff for a timed out read without converting HTTP refusals into timeouts',async()=>{
 const {OpenFreeMapTimeoutCooldown}=await import('./openFreeMapRoadData');
 vi.useFakeTimers();
 try{
  let attempt=0;
  const f=vi.fn(async(url:unknown,init?:RequestInit)=>{
   if(String(url).endsWith('/planet'))return new Response(JSON.stringify({maxzoom:14,tiles:['https://tiles.openfreemap.org/planet/rev/{z}/{x}/{y}.pbf']}));
   if(++attempt===1)return new Promise<Response>((_,reject)=>init?.signal?.addEventListener('abort',()=>reject(new DOMException('aborted','AbortError')),{once:true}));
   if(attempt===2)return new Response(null,{status:404});
   return new Response(new Uint8Array(fixture()));
  });
  const r=createOpenFreeMapRoadRepository(f as typeof fetch);
  const read=()=>r.observeTiles([[14552,6451]],'rev');
  const first=expect(read()).rejects.toMatchObject({name:'TimeoutError'});
  await vi.advanceTimersByTimeAsync(20001);await first;
  await expect(read()).rejects.toBeInstanceOf(OpenFreeMapTimeoutCooldown);
  expect(f).toHaveBeenCalledTimes(2);
  await vi.advanceTimersByTimeAsync(60000);
  await expect(read()).rejects.toThrow('404');
  const backoff=await read().catch(e=>e);
  expect(backoff).toBeInstanceOf(Error);
  expect(backoff).not.toBeInstanceOf(OpenFreeMapTimeoutCooldown);
  expect(backoff.message).toContain('再試行');
  expect(f).toHaveBeenCalledTimes(3);
  await vi.advanceTimersByTimeAsync(60000);
  await expect(read()).resolves.toMatchObject([{revision:'rev',x:14552,y:6451}]);
  expect(f).toHaveBeenCalledTimes(4);
 }finally{vi.useRealTimers();}
});
