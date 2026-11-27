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

test("saved grouping stays owner scoped and does not compare every built pair",()=>{
 let ownerReads=0;
 const count=1280;
 const footprints=Array.from({length:count},(_,i):StationFootprint=>({
  ...a,id:String(i),name:String(i),
  get owner(){ownerReads++;return String(i%2);}
 }));
 const ids=footprints.map(p=>p.id);
 const groups=stationGroupViews(footprints,[{station_id:"0",members:ids}]);
 expect(groups.map(group=>[group.id,group.members.length])).toEqual([["0",640],["1",640]]);
 expect(ownerReads).toBeLessThan(count*4);
});

test("an intersecting planned platform can join distinct saved groups without renaming either built canonical station first",()=>{
 const builtNear={...a,id:"12",name:"既設２",lineId:"5",polygon:rect(-2,1,4,.3)};
 const builtRemote={...a,id:"11",name:"別ホーム",lineId:"7",polygon:rect(8,8,1,1)};
 const unaffected={...a,id:"13",name:"隣社",owner:"7"};
 const saved=[{station_id:"10",members:["10","11","13"]}];
 expect(stationGroupViews([builtRemote,unaffected,builtNear,a,b],saved).map(g=>[g.id,g.members.map(p=>p.id)]))
  .toEqual([["10",["10","11","12","planned:1"]],["13",["13"]]]);
});
