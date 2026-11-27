import {expect,test,vi} from 'vitest';
import {createOpenFreeMapRoadRepository,OFM_METADATA} from './openFreeMapRoadData';
import {createRoadDrawingEngine} from './roadDrawingEngine';
import {bindTramRoadSnap,reacquireTramRoadSeam,type AcceptedTramRoadSnap} from './reacquireTramRoadSeam';

const revision='rosenfu_webview_fixture_16';
const tile=(x:number)=>`${OFM_METADATA}/${revision}/14/${x}/6452.pbf`;
const old={latitude:35.6740144226818,longitude:139.74504232406616};
const next={latitude:35.67362222198714,longitude:139.75226283073425};
const regionAround=({latitude,longitude}:typeof old)=>({west:longitude-.0001,east:longitude+.0001,south:latitude-.0001,north:latitude+.0001});
const left='GkoKDnRyYW5zcG9ydGF0aW9ueAIogCASFxIEAAABARgCIg0J+DyIBBKMAbQBzAMAGgVjbGFzcyIHCgVtaW5vchoGb25ld2F5IgIgAQ==';
const right='GkYKDnRyYW5zcG9ydGF0aW9ueAIogCASExIEAAABARgCIgkJzwG8BQrMEwAaBWNsYXNzIgcKBW1pbm9yGgZvbmV3YXkiAiAB';

const fixture=({missing=false,disconnected=false,sourceRevision=revision}={})=>{
 // Shift the last two left-fragment source Y positions by +40 tile units.
 // This is a valid new synthetic MVT body with no overlap to the right.
 const shifted='GkoKDnRyYW5zcG9ydGF0aW9ueAIogCASFxIEAAABARgCIg0J+DyIBBKMAYQCzAMAGgVjbGFzcyIHCgVtaW5vchoGb25ld2F5IgIgAQ==';
 const bytes=(value:string)=>Uint8Array.from(atob(value),c=>c.charCodeAt(0));
 const fetcher=vi.fn(async(url:unknown)=>{
  if(url===OFM_METADATA)return Response.json({maxzoom:14,tiles:[`${OFM_METADATA}/${sourceRevision}/{z}/{x}/{y}.pbf`]});
  if(url===tile(14551).replace(revision,sourceRevision)&&!missing)return new Response(bytes(disconnected?shifted:left));
  if(url===tile(14552).replace(revision,sourceRevision))return new Response(bytes(right));
  return new Response(null,{status:404});
 });
 const repo=createOpenFreeMapRoadRepository(fetcher);
 const observe=vi.fn((...args:Parameters<typeof repo.observeTiles>)=>repo.observeTiles(...args));
 const read=vi.fn(async(...args:Parameters<typeof repo.readTileEvidenceBatch>)=>
   ({status:'ready' as const,evidence:await repo.readTileEvidenceBatch(...args)}));
 return {fetcher,repo,provider:{observeRouteTiles:observe,readTileEvidenceBatch:read},observe,read};
};

async function acceptedProof(first=fixture().repo.load(regionAround(old))):Promise<AcceptedTramRoadSnap>{
 const snapshot=await first;
 const initial=await createRoadDrawingEngine(snapshot.bundle,snapshot.tileLines)({point:old,radiusM:20,authoredTramCue:true,resolveAuthoredSeam:true});
 const proof=initial.kind==='point'?bindTramRoadSnap(snapshot,initial.roadSnap):undefined;
 if(!proof)throw Error('valid fixture lacks confirmed worker receipt');
 return proof;
}

test('old snapped start outside current viewport can recover exact two-tile seam from the shared provider without new fetch',async()=>{
 const {fetcher,repo,provider,observe,read}=fixture();
 const first=await repo.load(regionAround(old));
 expect(first.tileSnapshots.map(t=>t.bundle.source.source)).toEqual([tile(14551)]);
 const begin=createRoadDrawingEngine(first.bundle,first.tileLines);
 expect(await begin({point:old,radiusM:20,authoredTramCue:true})).toEqual({kind:'point',point:old});
 const current=await repo.load(regionAround(next));
 expect(current.tileSnapshots.map(t=>t.bundle.source.source)).toEqual([tile(14552)]);
 const worker=createRoadDrawingEngine(current.bundle,current.tileLines);
 expect(await worker({start:old,point:next,radiusM:20,authoredTramCue:true})).toEqual({kind:'free'});
 expect(await worker({start:old,point:next,radiusM:20,authoredTramCue:true,resolveAuthoredSeam:true})).toEqual({kind:'free'});
 expect(observe).not.toHaveBeenCalled();
 const ctrl=new AbortController();
 const proof=await acceptedProof(Promise.resolve(first));
 const before=fetcher.mock.calls.length;
 const route=await reacquireTramRoadSeam(current,old,proof,next,false,provider,()=>true,ctrl.signal);
 expect(observe).toHaveBeenCalledExactlyOnceWith([[14551,6452]],revision,ctrl.signal);
 expect(read).toHaveBeenCalledTimes(1);
 expect(fetcher).toHaveBeenCalledTimes(before);
 expect(route?.line[0]).toEqual(old);
 expect(route?.point).toEqual(next);
 expect(route?.line.at(-1)).toEqual(next);
 expect(route?.line.length).toBeGreaterThan(4);
});

test('only the previous missing cell may be fetched; unavailable and disconnected data do not create free chords',async()=>{
 const proof=await acceptedProof();
 for(const scenario of ['empty cache','404','disconnected']){
  const {fetcher,repo,provider}=fixture({missing:scenario==='404',disconnected:scenario==='disconnected'});
  const expected=scenario==='disconnected'?await acceptedProof(fixture({disconnected:true}).repo.load(regionAround(old))):proof;
  const current=await repo.load(regionAround(next));
  const ctrl=new AbortController(),run=reacquireTramRoadSeam(current,old,expected,next,false,provider,()=>true,ctrl.signal);
  if(scenario==='empty cache')expect(await run).toMatchObject({kind:'route',point:next});
  else await expect(run).rejects.toThrow();
  expect(fetcher.mock.calls.map(([url])=>url)).toEqual([OFM_METADATA,tile(14552),tile(14551)]);
 }
});

test('both endpoint cells observed, wider or stale scope never start targeted observation',async()=>{
 const proof=await acceptedProof();
 const {repo,provider,observe,read}=fixture();
 const full=await repo.load({west:old.longitude-.0001,east:next.longitude+.0001,south:next.latitude-.0001,north:old.latitude+.0001});
 const ctrl=new AbortController();
 expect(full.tileSnapshots.map(t=>t.bundle.source.source)).toEqual([tile(14551),tile(14552)]);
 expect(await reacquireTramRoadSeam(full,old,proof,next,false,provider,()=>true,ctrl.signal)).toBeNull();
 const current=await repo.load(regionAround(next));
 expect(await reacquireTramRoadSeam(current,{...old,longitude:old.longitude-.04},proof,next,false,provider,()=>true,ctrl.signal)).toBeNull();
 expect(await reacquireTramRoadSeam(current,old,proof,next,false,provider,()=>false,ctrl.signal)).toBeNull();
 expect(observe).not.toHaveBeenCalled();
 expect(read).not.toHaveBeenCalled();
});

test('rejects a late result and exact endpoint mismatch before returning any geometry',async()=>{
 const proof=await acceptedProof();
 const {repo,provider,read}=fixture();
 const current=await repo.load(regionAround(next));
 const ctrl=new AbortController();
 expect(await reacquireTramRoadSeam(current,old,proof,{...next,latitude:next.latitude+.00002},true,provider,()=>true,ctrl.signal)).toBeNull();
 expect(read).not.toHaveBeenCalled();
 const late={...provider,observeRouteTiles:async(...args:Parameters<typeof provider.observeRouteTiles>)=>{
  const found=await provider.observeRouteTiles(...args);
  ctrl.abort();
  return found;
 }};
 expect(await reacquireTramRoadSeam(current,old,proof,next,false,late,()=>true,ctrl.signal)).toBeNull();
 expect(read).not.toHaveBeenCalled();
});

test('revision, altered prior digest and wrong source occurrence cannot replace the accepted start',async()=>{
 const proof=await acceptedProof();
 const signal=new AbortController().signal;
 const changed=fixture({sourceRevision:'later_source'});
 const snapshot=await changed.repo.load(regionAround(next));
 expect(await reacquireTramRoadSeam(snapshot,old,proof,next,false,changed.provider,()=>true,signal)).toBeNull();
 expect(changed.observe).not.toHaveBeenCalled();
 const altered=fixture({disconnected:true});
 const resident=await altered.repo.load(regionAround(next));
 await expect(reacquireTramRoadSeam(resident,old,proof,next,false,altered.provider,()=>true,signal)).rejects.toThrow('以前の道路データが変わりました');
 expect(altered.read).not.toHaveBeenCalled();
 const normal=fixture();
 const current=await normal.repo.load(regionAround(next));
 const wrong={...proof,pick:{...proof.pick,segmentIndex:1}};
 await expect(reacquireTramRoadSeam(current,old,wrong,next,false,normal.provider,()=>true,signal)).rejects.toThrow('以前の起点');
});
