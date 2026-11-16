import {expect,test} from 'vitest';
import {ofmEndpointPick} from './ofmEndpointPick';
import type {TileRoadLine} from './ofmBoundaryContinuations';
const line={id:'a',coordinates:[[139,35],[139.001,35],[139.002,35]]} as unknown as TileRoadLine;
const pick={fragmentId:'a',segmentIndex:0,fraction:.5};
test('fraction correction follows exact source geometry without mutation',()=>{const before=JSON.stringify(line);expect(ofmEndpointPick([line],pick,{longitude:139.0015,latitude:35})).toEqual({fragmentId:'a',segmentIndex:1,fraction:expect.closeTo(.5,7)});expect(JSON.stringify(line)).toBe(before);});
test('vertex, last endpoint and reversed source are supported',()=>{expect(ofmEndpointPick([line],pick,{longitude:139.001,latitude:35})).toMatchObject({segmentIndex:1,fraction:0});expect(ofmEndpointPick([line],pick,{longitude:139.002,latitude:35})).toMatchObject({segmentIndex:1,fraction:1});expect(ofmEndpointPick([{...line,coordinates:[...line.coordinates].reverse()}],pick,{longitude:139.0015,latitude:35})?.fraction).toBeCloseTo(.5);});
test('off-source, beyond extent and ambiguous crossings are rejected',()=>{expect(ofmEndpointPick([line],pick,{longitude:139.001,latitude:35.00001})).toBeNull();expect(ofmEndpointPick([line],pick,{longitude:139.003,latitude:35})).toBeNull();const crossed={...line,coordinates:[[0,0],[1,1],[0,1],[1,0]] as [number,number][]};expect(ofmEndpointPick([crossed],pick,{longitude:.5,latitude:.5})).toBeNull();});
