import {afterEach,expect,it,vi} from 'vitest';
const mock=vi.hoisted(()=>({tiles:vi.fn()}));
vi.mock('./openFreeMapRoadData',()=>({createOpenFreeMapRoadRepository:mock.tiles}));
import {createRoadProvider} from './roadProvider';
const bounds={west:139,south:35,east:139.01,north:35.01};
afterEach(()=>vi.clearAllMocks());
it('selects on-demand vector roads and preserves requested bounds',async()=>{
 const snapshot={coverageVerified:false};const load=vi.fn().mockResolvedValue(snapshot);mock.tiles.mockReturnValue({load});
 const provider=createRoadProvider();expect(provider.name).toBe('OpenFreeMap / OpenStreetMap');expect(await provider.load(bounds)).toBe(snapshot);expect(load).toHaveBeenCalledWith(bounds,undefined);
});
it('does not switch sources or turn missing tiles into empty roads',async()=>{
 const load=vi.fn().mockRejectedValue(Error('tile unavailable'));mock.tiles.mockReturnValue({load});
 await expect(createRoadProvider().load(bounds)).rejects.toThrow('tile unavailable');expect(load).toHaveBeenCalledTimes(1);expect(mock.tiles).toHaveBeenCalledTimes(1);
});
it('preserves initialization failures instead of starting a pack download',()=>{mock.tiles.mockImplementation(()=>{throw Error('invalid tile provider');});expect(()=>createRoadProvider()).toThrow('invalid tile provider');});
it('shares load and evidence through one repository, preserving refs and cancellation',async()=>{
 const snapshot={tileSnapshots:[{revision:'pinned'}]},evidence={rawTiles:[],bindings:[]};const load=vi.fn().mockResolvedValue(snapshot),readTileEvidenceBatch=vi.fn().mockResolvedValue(evidence);mock.tiles.mockReturnValue({load,readTileEvidenceBatch});const provider=createRoadProvider(),c=new AbortController();
 expect(await provider.load(bounds,c.signal)).toBe(snapshot);expect(load).toHaveBeenCalledWith(bounds,c.signal);expect(await provider.readTileEvidenceBatch([],c.signal)).toBe(evidence);expect(readTileEvidenceBatch).toHaveBeenCalledWith([],c.signal);expect(mock.tiles).toHaveBeenCalledTimes(1);
});
