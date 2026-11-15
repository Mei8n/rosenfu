// @vitest-environment jsdom
import {afterEach,expect,it,vi} from 'vitest';
const mock=vi.hoisted(()=>({release:null as null|{manifestSha256:string;name:string},pack:vi.fn(),overpass:vi.fn()}));
vi.mock('./roadPackRelease',()=>({get ROAD_PACK_RELEASE(){return mock.release;}}));
vi.mock('./roadPackRepository',()=>({createRoadPackRepository:mock.pack}));
vi.mock('./automaticRoadData',()=>({createRoadRepository:mock.overpass}));
import {createRoadProvider} from './roadProvider';
const bounds={west:139,south:35,east:139.01,north:35.01};
afterEach(()=>{vi.clearAllMocks();mock.release=null;});
it('uses bounded Overpass only when no pack release is configured',()=>{mock.overpass.mockReturnValue({load:vi.fn()});expect(createRoadProvider().name).toBe('OpenStreetMap / Overpass');expect(mock.overpass).toHaveBeenCalledTimes(1);expect(mock.pack).not.toHaveBeenCalled();});
it('selects the pinned same-origin release and never falls back on missing coverage',async()=>{mock.release={manifestSha256:'a'.repeat(64),name:'Tokyo OSM snapshot'};const load=vi.fn().mockRejectedValue(Error('missing cell'));mock.pack.mockReturnValue({load});const provider=createRoadProvider();expect(provider.name).toBe(mock.release.name);expect(mock.pack.mock.calls[0][0].href).toBe(new URL('/road-data/',location.origin).href);expect(mock.pack.mock.calls[0][1]).toBe(mock.release.manifestSha256);await expect(provider.load(bounds)).rejects.toThrow('missing cell');expect(mock.overpass).not.toHaveBeenCalled();});
it('does not fall back when pack initialization fails',()=>{mock.release={manifestSha256:'invalid',name:'pack'};mock.pack.mockImplementation(()=>{throw Error('invalid hash');});expect(()=>createRoadProvider()).toThrow('invalid hash');expect(mock.overpass).not.toHaveBeenCalled();});
