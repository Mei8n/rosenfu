import {expect,test} from 'vitest';
import {automaticStationName} from './automaticStationName';
import {createReferenceRepository,type ReferencePack} from './referenceStations';
const point={latitude:35,longitude:139};
const pack:ReferencePack={format:'rosenfu-reference-stations',version:1,dataset:{id:'test',revision:'1',source:'fixture',license:'test',bounds:{west:138,east:140,south:34,north:36}},stations:[{osmType:'node',osmId:'1',name:'中央',kind:'station',...point}]};
test('offline missing data always supplies a usable unique default',()=>{expect(automaticStationName(point,null,[]).name).toBe('新駅');expect(automaticStationName(point,null,['新駅','新駅 2']).name).toBe('新駅 3');});
test('nearest usable local name is automatic; missing coverage falls back',()=>{const repo=createReferenceRepository(pack);expect(automaticStationName(point,repo,[]).name).toBe('中央');expect(automaticStationName(point,repo,['中央']).name).toBe('中央 2');expect(automaticStationName({latitude:30,longitude:130},repo,[]).name).toBe('新駅');expect(automaticStationName({latitude:35.5,longitude:139},repo,[]).name).toBe('新駅');});

test('duplicate suffix keeps long Unicode names within the existing 80-unit limit',()=>{const name='🚉'.repeat(40);const repo=createReferenceRepository({...pack,stations:[{...pack.stations[0],name}]});const result=automaticStationName(point,repo,[name]);expect(result.name.length).toBeLessThanOrEqual(80);expect(result.name.endsWith(' 2')).toBe(true);expect(result.name.includes('\uFFFD')).toBe(false);});
