import {expect,it} from 'vitest';
import {acceptableBusStopName,appendBusStopSuggestions,pickBusStopOccurrences,verifyBusStopPlacement} from './ordinaryBusStops';
import type {OrdinaryBusPath} from './ordinaryBusDraft';

const a={latitude:35.67504,longitude:139.748},b={latitude:a.latitude,longitude:139.752},
 c={latitude:a.latitude-.001,longitude:139.748},centre={latitude:a.latitude,longitude:139.75};
const cam={latitude:a.latitude,longitude:139.75,zoom:16};
const keys=['a','b','c'].map(value=>value.repeat(64));
const path:OrdinaryBusPath={rule:'ofm_ground_bus_z14_v1',tileKey:'https://tiles.openfreemap.org/planet/r1/14/14552/6452.pbf',
 tileRawSha256:'1'.repeat(64),respectOneway:false,start:{feature:0,line:0,vertex:0},end:{feature:0,line:0,vertex:1}};

it('only returns near route interiors and preserves each coincident traversal choice',()=>{
 const preview={points:[a,b,a,c],lengthM:900,stopOccurrenceKeys:keys};
 expect(pickBusStopOccurrences(preview,centre,cam,1).map(c=>[c.segment,c.fractionQ,c.occurrenceKey])).toEqual([
  [0,500000,keys[0]],[1,500000,keys[1]],
 ]);
 expect(pickBusStopOccurrences(preview,a,cam,1)).toEqual([]);
 expect(pickBusStopOccurrences(preview,{...centre,latitude:centre.latitude+.001},cam,1)).toEqual([]);
});

it('fails closed if authoritative occurrence or saved stop receipts mismatch',()=>{
 const proof={...path,interiorStops:[{id:'s-x',name:'中央',occurrenceKey:keys[0],fractionQ:500000}]};
 const receipt={points:[a,b],lengthM:370,stopOccurrenceKeys:[keys[0]],interiorStops:[
  {id:'s-x',stopId:'trusted-id',name:'中央',point:centre,servesReturn:false},
 ]};
 expect(verifyBusStopPlacement(proof,receipt).interiorStops).toEqual(receipt.interiorStops);
 expect(()=>verifyBusStopPlacement(proof,{...receipt,stopOccurrenceKeys:[keys[1]]})).toThrow('確認できません');
 expect(()=>verifyBusStopPlacement(proof,{...receipt,interiorStops:[{...receipt.interiorStops[0],point:b}]})).toThrow('確認できません');
 expect(()=>verifyBusStopPlacement(proof,{...receipt,interiorStops:[{...receipt.interiorStops[0],servesReturn:true}]})).toThrow('確認できません');
 expect(acceptableBusStopName('駅'.repeat(67))).toBe(false);
 expect(acceptableBusStopName('A\u0000B')).toBe(false);
});


it('automatic candidates only add within the same verified traversals and cannot replace manual stops',()=>{
 const existing={id:'manual-1',name:'中央',occurrenceKey:keys[0],fractionQ:500000};
 const added={id:'auto-1',name:'自動 1',occurrenceKey:keys[1],fractionQ:500000};
 const proof={...path,interiorStops:[existing]},preview={points:[a,b,c],lengthM:700,stopOccurrenceKeys:keys.slice(0,2)};
 expect(appendBusStopSuggestions(proof,preview,[added]).interiorStops).toEqual([existing,added]);
 expect(appendBusStopSuggestions(proof,preview,[])).toBe(proof);
 for(const bad of [{...added,id:existing.id},{...added,occurrenceKey:keys[2]}, {...added,occurrenceKey:keys[0]}, {...added,fractionQ:1_000_000}]){
  expect(()=>appendBusStopSuggestions(proof,preview,[bad])).toThrow('自動配置');
 }
});

it('maps clipped first/last segments onto the exact saved orientation and refuses endpoint cuts',()=>{
 const selected=keys.slice(0,2),bounds=[200_000,700_000] as const;
 const clipped={points:[a,b,{latitude:b.latitude-.001,longitude:b.longitude}],lengthM:700,
  stopOccurrenceKeys:selected,stopTargetTerminalCuts:bounds};
 const proof={...path,fractionalTerminals:[200_000,700_000] as const,interiorStops:[
  {id:'s-a',name:'中間',occurrenceKey:keys[0],fractionQ:600_000},
 ]};
 const receipt={...clipped,interiorStops:[{id:'s-a',stopId:'s-a:physical',name:'中間',point:centre,servesReturn:false}]};
 expect(verifyBusStopPlacement(proof,receipt).stopTargetTerminalCuts).toEqual(bounds);
 expect(pickBusStopOccurrences(clipped,centre,cam,1).map(p=>p.fractionQ)).toEqual([600_000]);
 const second={latitude:b.latitude-.0005,longitude:b.longitude};
 // Latitude midpoint is not exactly halfway in the Mercator projection.
 expect(Math.abs((pickBusStopOccurrences(clipped,second,cam,1)[0]?.fractionQ??0)-350_000)).toBeLessThanOrEqual(2);
 for(const invalid of [100_000,200_000])expect(()=>verifyBusStopPlacement(
  {...proof,interiorStops:[{...proof.interiorStops[0],fractionQ:invalid}]},receipt
 )).toThrow('確認できません');
 expect(()=>verifyBusStopPlacement(proof,{...receipt,stopTargetTerminalCuts:undefined})).toThrow('確認できません');
});
