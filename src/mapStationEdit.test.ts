import {expect,test,vi} from 'vitest';
import {editStationOrder} from './mapStationEdit';
const found=(ids:string[])=>({status:'unique' as const,candidates:[{stations:ids,segments:[]}],truncated:false,shortest_candidate_index:0});
test('physical path cost picks insertion position and never reorders existing constraints',async()=>{
 const lookup=vi.fn(async(ids:string[])=>found(ids));
 const chosen=await editStationOrder(['a','c'],'b',lookup,p=>p.stations.join('')==='abc'?5:20);
 expect(chosen.ids).toEqual(['a','b','c']);expect(lookup).toHaveBeenCalledTimes(3);
});
test('clicking either endpoint shortens draft while fewer than two stations is refused',async()=>{
 expect((await editStationOrder(['a','b','c'],'a',async ids=>found(ids),()=>1)).ids).toEqual(['b','c']);
 expect((await editStationOrder(['a','b','c'],'c',async ids=>found(ids),()=>1)).ids).toEqual(['a','b']);
 await expect(editStationOrder(['a','b'],'a',async ids=>found(ids),()=>1)).rejects.toThrow('2駅');
});
test('unknown/incomplete exploration never claims an optimal insertion',async()=>{
 await expect(editStationOrder(['a','b'],'c',async()=>({...found(['a','b']),truncated:true}),()=>1)).rejects.toThrow('確認しきれ');
});
