import {describe,it,expect,vi} from 'vitest';
import {createWaterSource,createWaterRepository,decodeWaterTile,waterTileBounds} from './automaticWaterData';
describe('water acquisition fails closed',()=>{
 it('rejects a server returning the whole archive',async()=>{const source=createWaterSource(vi.fn(async()=>new Response('archive',{status:200})));await expect(source.getBytes(0,8)).rejects.toThrow('HTTP 200');});
 it('requires matching content range and length',async()=>{const fetcher=vi.fn(async()=>new Response(new Uint8Array(8),{status:206,headers:{'content-range':'bytes 0-7/100'}}));const source=createWaterSource(fetcher);expect((await source.getBytes(0,8)).data.byteLength).toBe(8);await expect(source.getBytes(8,8)).rejects.toThrow();});
 it('does not interpret an empty tile as dry land',()=>{expect(()=>decodeWaterTile(new ArrayBuffer(0),14550,6460,14)).toThrow('カバレッジ');});
 it('bounds area and rejects invalid range sizes before network',async()=>{const fetcher=vi.fn();await expect(createWaterSource(fetcher).getBytes(0,1e10)).rejects.toThrow();await expect(createWaterRepository(fetcher).load({west:130,east:140,south:30,north:40})).rejects.toThrow();expect(fetcher).not.toHaveBeenCalled();});
 it('adjacent tile boundaries meet exactly',()=>{const a=waterTileBounds(14550,6460,14),b=waterTileBounds(14551,6460,14);expect(a.east).toBe(b.west);});
});
