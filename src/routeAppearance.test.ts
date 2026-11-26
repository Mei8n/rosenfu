import { expect,test } from "vitest";
import {routeAppearance} from "./routeAppearance";
const world={stations:[{id:"s1"},{id:"s2"},{id:"s3"}],tracks:[{id:"t",from:"s1",to:"s2"}],routes:[] as {id:string;start:string}[],alignments:[{id:"a",points:[{latitude:35,longitude:139},{latitude:35,longitude:139.01},{latitude:35,longitude:139.02}]}],station_placements:[{station_id:"s1",alignment_id:"a",position:.25},{station_id:"s2",alignment_id:"a",position:.75},{station_id:"s3",alignment_id:"a",position:.5}]};
test("unassigned equipment and service-only usage do not acquire route colours",()=>{
 const result=routeAppearance({...world,services:[{tracks:["t"]}]} as typeof world);
 expect(result.stations.size).toBe(0);expect(result.alignmentSections.size).toBe(0);
});
test("missing physical route metadata never infers geometry or membership",()=>{
 const result=routeAppearance({...world,routes:[{id:"r",start:"s1"}]});
 expect(result.stations.size).toBe(0);expect(result.alignmentSections.size).toBe(0);
});
test("modern route paths use explicit membership, not all stations physically on the line",()=>{
 const result=routeAppearance({...world,routes:[{id:"r",start:"s1"}],route_paths:[{route_id:"r",segments:[{kind:"alignment",alignment_id:"a",from:"s2",to:"s1"}],member_stations:["s1","s2"]}]});
 expect(result.stations.has("s3")).toBe(false);expect(result.alignmentSections.get("a")![0]).toHaveLength(3);
});
test("orphan sidecars and invalid references never colour unrelated infrastructure",()=>{
 const result=routeAppearance({...world,route_paths:[{route_id:"missing",segments:[{kind:"alignment",alignment_id:"a",from:"s1",to:"s2"}],member_stations:["s1"]}]});
 expect(result.stations.size).toBe(0);
});

test("candidate sections share clipping without changing saved route membership",async()=>{
 const {routeSegmentSections}=await import('./routeAppearance');
 const before=JSON.stringify(world);
 const selected=routeSegmentSections(world,[{kind:'alignment',alignment_id:'a',from:'s1',to:'s2'}]);
 expect(selected.get('a')![0]).toHaveLength(3);
 expect(selected.get('a')![0][0].longitude).toBeCloseTo(139.005);
 expect(selected.get('a')![0].at(-1)!.longitude).toBeCloseTo(139.015);
 expect(routeSegmentSections(world,[{kind:'alignment',alignment_id:'missing',from:'s1',to:'s2'}]).size).toBe(0);
 expect(routeSegmentSections(world,[]).size).toBe(0);
 expect(JSON.stringify(world)).toBe(before);
 expect(routeAppearance(world).alignmentSections.size).toBe(0);
});
