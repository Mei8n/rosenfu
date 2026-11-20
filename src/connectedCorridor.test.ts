import {expect,test} from "vitest";
import {corridorSpans,corridorStations} from "./connectedCorridor";
import {resolveServicePath,sameSegment} from "./servicePaths";
import {routeAppearance} from "./routeAppearance";
import type {Segment} from "./RouteCandidatePreview";
const links=[{a:{alignment_id:"a",endpoint:"end" as const},b:{alignment_id:"b",endpoint:"start" as const}},{a:{alignment_id:"b",endpoint:"end" as const},b:{alignment_id:"c",endpoint:"end" as const}}];
const segment:Extract<Segment,{kind:"connected_alignments"}>={kind:"connected_alignments",from:"1",to:"3",connections:links};
const world={alignment_connections:links,station_placements:[{station_id:"1",alignment_id:"a",position:.5},{station_id:"2",alignment_id:"b",position:.4},{station_id:"3",alignment_id:"c",position:.3}],tracks:[],stations:[{id:"1"},{id:"2"},{id:"3"}],alignments:["a","b","c"].map((id,i)=>({id,points:[{latitude:35,longitude:139+i*.01},{latitude:35,longitude:139+(i+1)*.01}]})),routes:[{id:"r",start:"1",tracks:[]}],route_paths:[{route_id:"r",segments:[segment],member_stations:["1","3"]}]};
test("corridor services retain leg keys and intermediate stations across reverse target",()=>{
 expect(corridorSpans(segment,world)).toEqual([{alignmentId:"a",begin:.5,finish:1},{alignmentId:"b",begin:0,finish:1},{alignmentId:"c",begin:1,finish:.3}]);
 expect(corridorStations(segment,world)).toEqual(["2","3"]);
 expect(resolveServicePath("1",[{key:"7",segment}],world).map(o=>o.reference)).toEqual([{kind:"start"},{kind:"on_leg",leg_key:"7",station_id:"2"},{kind:"on_leg",leg_key:"7",station_id:"3"}]);
 expect(sameSegment(segment,structuredClone(segment))).toBe(true);
 expect(sameSegment(segment,{...segment,connections:[links[1],links[0]]})).toBe(false);
 const appearance=routeAppearance(world);
 expect([...appearance.alignmentSections.keys()]).toEqual(["a","b","c"]);
 expect([...appearance.stations]).toEqual(["1","3"]);
});
test("missing links cannot silently paint or resolve a disconnected corridor",()=>{
 expect(()=>corridorStations(segment,{...world,alignment_connections:[]})).toThrow();
 expect(routeAppearance({...world,alignment_connections:[]}).alignmentSections.size).toBe(0);
});

test("a closed corridor returns to the opposite end of the source without overlapping its first span",()=>{
 const connections=[links[0],{a:{alignment_id:"b",endpoint:"end" as const},b:{alignment_id:"a",endpoint:"start" as const}}];
 const ring={...world,alignment_connections:connections,station_placements:[{station_id:"1",alignment_id:"a",position:.8},{station_id:"3",alignment_id:"a",position:.2}]};
 expect(corridorStations({...segment,connections},ring)).toEqual(["3"]);
 expect(()=>corridorStations({...segment,connections},{...ring,station_placements:[{station_id:"1",alignment_id:"a",position:.2},{station_id:"3",alignment_id:"a",position:.8}]})).toThrow();
});
