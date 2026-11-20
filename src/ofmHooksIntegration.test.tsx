// @vitest-environment jsdom
import {act,cleanup,renderHook} from '@testing-library/react';
import {afterEach,expect,test,vi} from 'vitest';
const mocks=vi.hoisted(()=>({load:vi.fn(),read:vi.fn(),draft:vi.fn()}));
vi.mock('./roadProvider',()=>({createRoadProvider:()=>({name:'fixture',load:mocks.load,readTileEvidenceBatch:mocks.read})}));
vi.mock('./ofmSharedEvidenceDraft',()=>({prepareSharedOfmEvidenceDraft:mocks.draft}));
import {useAutomaticRoad} from './useAutomaticRoad';
import {useOfmSeamCopy} from './useOfmSeamCopy';
const bounds={west:139.7,east:139.72,south:35.62,north:35.64},source='https://tiles.openfreemap.org/planet/rev/14/14552/6451.pbf',sha='a'.repeat(64);
const frozen=[{tileLines:[],bundle:{source:{source,license:'ODbL-1.0',attribution:'OSM'},sourceSha256:sha}}] as any;
const required=[{x:14552,y:6451}],pick={fragmentId:'one',segmentIndex:0,fraction:0};
const evidence={rawTiles:[{sha256:sha,bytes:new Uint8Array([1])}],bindings:[{key:source,url:source,revision:'rev',z:14,x:14552,y:6451,rawSha256:sha,decoderRule:'ofm_transportation_z14_v1',license:'ODbL-1.0',attribution:'OSM'}]};
afterEach(()=>{cleanup();vi.useRealTimers();vi.clearAllMocks();});
async function setup(){vi.useFakeTimers();mocks.load.mockResolvedValue({roads:[],bundle:{}});mocks.draft.mockResolvedValue({result:'draft'});let renders=0;const h=renderHook(({world})=>{if(++renders>30)throw Error('render loop');const roads=useAutomaticRoad(bounds,true,world);const seam=useOfmSeamCopy({enabled:true,world,tool:'tracks',frozenTiles:frozen,requiredTiles:required,start:pick,end:pick,readTileEvidenceBatch:roads.readTileEvidenceBatch});return {roads,seam};},{initialProps:{world:1}});await act(async()=>{await h.result.current.roads.ensureCurrent();});return {h,renders:()=>renders};}
test('actual hook pair survives loading/result renders and completes preparation',async()=>{const {h,renders}=await setup();mocks.read.mockResolvedValue(evidence);await act(async()=>{expect(await h.result.current.seam.prepare()).toEqual({result:'draft'});});expect(h.result.current.seam.state.status).toBe('ready');expect(renders()).toBeLessThan(10);expect(mocks.read).toHaveBeenCalledTimes(1);});
test('actual hook pair aborts evidence and rejects late result when world changes',async()=>{const {h}=await setup();let finish!:(v:unknown)=>void,signal:AbortSignal|undefined;mocks.read.mockImplementation((_:unknown,s:AbortSignal)=>{signal=s;return new Promise(r=>finish=r);});let pending!:Promise<unknown>;act(()=>{pending=h.result.current.seam.prepare();});h.rerender({world:2});expect(signal?.aborted).toBe(true);await act(async()=>{finish(evidence);expect(await pending).toBeNull();});expect(h.result.current.seam.state.status).toBe('idle');expect(mocks.draft).not.toHaveBeenCalled();});
