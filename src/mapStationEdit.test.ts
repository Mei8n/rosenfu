import {expect,test,vi} from 'vitest';
import {editStationOrder,replaceStationEndpoint} from './mapStationEdit';
const found=(ids:string[])=>({status:'unique' as const,candidates:[{stations:ids,segments:[]}],truncated:false,shortest_candidate_index:0});
test('physical path cost picks insertion position and never reorders existing constraints',async()=>{
 const lookup=vi.fn(async(ids:string[])=>found(ids));
 const chosen=await editStationOrder(['a','c'],'b',lookup,p=>p.stations.join('')==='abc'?5:20);
 expect(chosen.ids).toEqual(['a','b','c']);expect(lookup).toHaveBeenCalledTimes(1);
});
test('explicit endpoint replacement can shorten, but intermediate edits cannot move endpoints',async()=>{
 expect(replaceStationEndpoint(['a','b','c'],'start','b')).toEqual(['b','c']);
 expect(replaceStationEndpoint(['a','b','c'],'end','b')).toEqual(['a','b']);
 expect(replaceStationEndpoint(['a','b','c'],'start','d')).toEqual(['d','b','c']);
 expect(()=>replaceStationEndpoint(['a','b'],'start','b')).toThrow('同じ駅');
 await expect(editStationOrder(['a','b'],'a',async ids=>found(ids),()=>1)).rejects.toThrow('個別');
});
test('unknown/incomplete exploration never claims an optimal insertion',async()=>{
 await expect(editStationOrder(['a','b'],'c',async()=>({...found(['a','b']),truncated:true}),()=>1)).rejects.toThrow('確認しきれ');
});
