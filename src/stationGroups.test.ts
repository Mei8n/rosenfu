import {expect,test} from "vitest";
import {stationGroupViews,footprintsIntersect,type StationFootprint} from "./stationGroups";
const rect=(x:number,y:number,w:number,h:number)=>[{longitude:x,latitude:y},{longitude:x+w,latitude:y},{longitude:x+w,latitude:y+h},{longitude:x,latitude:y+h}];
const a:StationFootprint={id:"10",name:"既設",owner:"2",lineId:"3",polygon:rect(-3,-.2,6,.4),planned:false};
const b:StationFootprint={id:"planned:1",name:"新駅",owner:"2",lineId:"plan:4",polygon:rect(-.2,-3,.4,6),planned:true};
test("intersecting separate-line planned platform joins existing station, preserving canonical name",()=>{
 const groups=stationGroupViews([a,b]);expect(groups).toHaveLength(1);expect(groups[0].name).toBe("既設");expect(groups[0].members).toHaveLength(2);
 expect(footprintsIntersect(a.polygon,b.polygon)).toBe(true);
});
test("nearby nonoverlapping, other owner and same line are never auto merged",()=>{
 for(const changed of [{...b,polygon:rect(4,4,1,1)},{...b,owner:"9"},{...b,lineId:"3"}])expect(stationGroupViews([a,changed])).toHaveLength(2);
});
test("built membership is explicit; old worlds are not silently regrouped",()=>{
 const built={...b,id:"11",planned:false};expect(stationGroupViews([a,built])).toHaveLength(2);
 expect(stationGroupViews([a,built],[{station_id:"10",members:["10","11"]}])).toHaveLength(1);
});
test("planned groups are derived after drag/delete and have deterministic names",()=>{
 const one={...a,id:"planned:2",name:"編集駅",planned:true};
 expect(stationGroupViews([one,b])[0].name).toBe("新駅");
 expect(stationGroupViews([one,{...b,polygon:rect(4,4,1,1)}])).toHaveLength(2);
 expect(stationGroupViews([one])).toHaveLength(1);
});
