import {expect,test} from 'vitest';
import {nearbyTransferStations,type TransferRoad} from './busTransferCandidates';

const stop={id:'stop',edge_id:'edge',position:0.5};
const road:TransferRoad={
 edges:[{id:'edge',from:'a',to:'b',source:'fixture',revision:'1',bus_access:'allowed',tram_access:'unknown',
  geometry:[{latitude:35,longitude:139},{latitude:35.01,longitude:139},{latitude:35.01,longitude:139.01}]}],
 bus_routes:[{id:'route',owner:'2',name:'線',edges:['edge'],stops:[{stop_id:'stop',edge_index:0}]}],
};
test('uses the stop distance along committed road geometry and returns only nearby coordinates in order',()=>{
 const stations=[
  {id:'midpoint-of-endpoints',name:'直線区間の中間',latitude:35.005,longitude:139.005},
  {id:'near',name:'付近',latitude:35.009,longitude:139.002},
  {id:'bend',name:'曲がり角',latitude:35.01,longitude:139},
  {id:'broken',name:'位置不明',latitude:35,longitude:NaN},
  {id:'too-far',name:'範囲外',latitude:36,longitude:139},
 ];
 const items=nearbyTransferStations(stations,road,stop);
 expect(items.map(item=>item.id)).toEqual(['bend','near','midpoint-of-endpoints']);
 expect(items[0].distanceM).toBeGreaterThan(50);
 expect(items[0].distanceM).toBeLessThan(150);
});
test('does not guess an orphan stop or interpret an invalid position as a road endpoint',()=>{
 const station={id:'near',name:'付近',latitude:35.009,longitude:139.002};
 expect(nearbyTransferStations([station],road,{...stop,id:'detached'})).toEqual([]);
 expect(nearbyTransferStations([station],road,{...stop,position:-1})).toEqual([]);
 expect(nearbyTransferStations([station],{...road,edges:[]},stop)).toEqual([]);
});
test('caps suggestions at four and skips values that are not positions of built stations',()=>{
 const stations=Array.from({length:7},(_,i)=>({id:String(i),name:'駅'+i,latitude:35.009,longitude:139+i*0.0001}));
 expect(nearbyTransferStations([{id:'unlocated',name:'位置なし'},...stations],road,stop).map(entry=>entry.id))
  .toEqual(['0','1','2','3']);
 expect(nearbyTransferStations(stations,{...road,bus_routes:[{...road.bus_routes[0],stops:[{stop_id:'stop',edge_index:2}]}]},stop))
  .toEqual([]);
});
