import { expect,test } from "vitest";
import {routeAppearance} from "./routeAppearance";
const world={stations:[{id:"s1"},{id:"s2"},{id:"s3"}],tracks:[{id:"t",from:"s1",to:"s2"}],routes:[] as {id:string;start:string;tracks:string[]}[],alignments:[{id:"a",points:[{latitude:35,longitude:139},{latitude:35,longitude:139.01},{latitude:35,longitude:139.02}]}],station_placements:[{station_id:"s1",alignment_id:"a",position:.25},{station_id:"s2",alignment_id:"a",position:.75},{station_id:"s3",alignment_id:"a",position:.5}]};
test("unassigned equipment and service-only usage do not acquire route colours",()=>{
 const result=routeAppearance({...world,services:[{tracks:["t"]}]} as typeof world);
 expect(result.stations.size).toBe(0);expect(result.tracks.size).toBe(0);expect(result.alignmentSections.size).toBe(0);
});
test("legacy routes associate stations and tracks and only their actual alignment interval",()=>{
 const result=routeAppearance({...world,routes:[{id:"r",start:"s1",tracks:["t"]}]});
 expect([...result.stations]).toEqual(["s1","s2"]);expect([...result.tracks]).toEqual(["t"]);
 const points=result.alignmentSections.get("a")![0];expect(points).toHaveLength(3);
 expect(points[0].longitude).toBeCloseTo(139.005,7);expect(points[2].longitude).toBeCloseTo(139.015,7);
});
test("modern route paths use explicit membership, not all stations physically on the line",()=>{
 const result=routeAppearance({...world,routes:[{id:"r",start:"s1",tracks:[]}],route_paths:[{route_id:"r",segments:[{kind:"alignment",alignment_id:"a",from:"s2",to:"s1"}],member_stations:["s1","s2"]}]});
 expect(result.stations.has("s3")).toBe(false);expect(result.alignmentSections.get("a")![0]).toHaveLength(3);
});
test("orphan sidecars and invalid references never colour unrelated infrastructure",()=>{
 const result=routeAppearance({...world,route_paths:[{route_id:"missing",segments:[{kind:"track",track_id:"t"}],member_stations:["s1"]}]});
 expect(result.stations.size).toBe(0);expect(result.tracks.size).toBe(0);
});
