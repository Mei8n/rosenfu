import {describe,expect,it} from "vitest";
import {destinationJoinError,mixedTramTracks,type JoinAlignment} from "./destinationJoin";
import type {TrackType} from "./TrackTypeControls";
const p=(longitude:number, latitude=35)=>({latitude,longitude});
const start={alignmentId:"1",endpoint:"end" as const};
const end={alignmentId:"2",endpoint:"start" as const};
const line=[p(139.01),p(139.02)];
const bound=(startType:TrackType,endType:TrackType,owner="3"):JoinAlignment[]=>[
 {id:"1",owner:"3",track_type:startType,points:[p(139),p(139.01)]},
 {id:"2",owner,track_type:endType,points:[p(139.02),p(139.03)]},
];

describe("built destination joining",()=>{
 it("uses the requested new track type to permit both directions of a tram/rail join",()=>{
   expect(mixedTramTracks("normal","tram")).toBe(true);
   expect(mixedTramTracks("tram","high_speed")).toBe(true);
   expect(mixedTramTracks("normal","high_speed")).toBe(false);
   expect(destinationJoinError(line,start,end,bound("normal","high_speed"),[],"3","tram")).toBeNull();
   expect(destinationJoinError(line,start,end,bound("tram","tram"),[],"3","normal")).toBeNull();
   expect(destinationJoinError(line,null,end,bound("tram","high_speed"),[],"3","tram")).toBeNull();
   expect(destinationJoinError(line,start,end,bound("normal","high_speed"),[],"3","normal")).toBe("線路種別が一致しません");
 });
 it("does not permit foreign owners, occupied endpoints, or incorrect tangent/coordinates",()=>{
   expect(destinationJoinError(line,start,end,bound("normal","high_speed","4"),[],"3","tram")).toMatch(/他社/);
   expect(destinationJoinError(line,start,end,bound("normal","high_speed"),[{a:{alignment_id:"2",endpoint:"start"},b:{alignment_id:"3",endpoint:"end"}}],"3","tram")).toMatch(/接続済み/);
   expect(destinationJoinError([p(139.01,35.01),p(139.02)],start,end,bound("normal","high_speed"),[],"3","tram")).toMatch(/接線/);
   expect(destinationJoinError([p(139.01),p(139.021)],start,end,bound("normal","high_speed"),[],"3","tram")).toMatch(/一致していません/);
 });
});
