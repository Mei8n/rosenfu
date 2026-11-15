import {expect,test} from "vitest";
import {stationTarget,stationContainsPoint} from "./stationTarget";
import {DEFAULT_CAMERA} from "./map";
const point={latitude:35.67504,longitude:139.75};
const line={id:"built",points:[{...point,longitude:139.73},{...point,longitude:139.77}]};
test("nearest candidate changes automatically between built and planned; invalid nearest never silently jumps",()=>{
 const planned={id:"plan",pathId:1,points:line.points.map(p=>({...p,latitude:p.latitude+.001}))};
 expect(stationTarget([line,planned],point,200,DEFAULT_CAMERA,1).target?.line.id).toBe("built");
 expect(stationTarget([line,planned],{...point,latitude:point.latitude+.001},200,DEFAULT_CAMERA,1).target?.line.id).toBe("plan");
 expect(stationTarget([line,{...planned,invalid:true}],{...point,latitude:point.latitude+.001},200,DEFAULT_CAMERA,1).error).toContain("敷設できません");
});
test("end platform clamps center, short line rejects and distant cursor has no target",()=>{
 const target=stationTarget([line],line.points[0],200,DEFAULT_CAMERA,1);
 expect(target.target!.position).toBeGreaterThan(0);expect(target.target!.polygon).toHaveLength(4);
 expect(stationTarget([{...line,points:[point,{...point,longitude:point.longitude+.0001}]}],point,200,DEFAULT_CAMERA,1).error).toContain("収まる");
 expect(stationTarget([line],{...point,latitude:36},200,DEFAULT_CAMERA,1).target).toBeNull();
});

test("new station lengths reject outside 2–20 cars without generating geometry",()=>{for(const length of [0,20,39.9,400.1,1000,NaN,Infinity]){const result=stationTarget([line],point,length,DEFAULT_CAMERA,1);expect(result.target).toBeNull();expect(result.error).toContain("2〜20");}for(const length of [40,231.5,400])expect(stationTarget([line],point,length,DEFAULT_CAMERA,1).error).toBe("");});


test("same-line station intervals reject duplicates while other lines remain available for future shared stations",()=>{
 const occupied=[{lineId:line.id,position:.5,lengthM:200}];
 expect(stationTarget([line],point,200,DEFAULT_CAMERA,1,occupied).error).toContain("重な");
 expect(stationTarget([{...line,id:"plan:1",pathId:1}],point,200,DEFAULT_CAMERA,1,occupied).error).toBe("");
 expect(stationTarget([line],{...point,longitude:point.longitude+.01},200,DEFAULT_CAMERA,1,occupied).error).toBe("");
 const polygon=stationTarget([line],point,200,DEFAULT_CAMERA,1).target!.polygon;
 expect(stationContainsPoint(polygon,point)).toBe(true);
 expect(stationContainsPoint(polygon,{...point,latitude:point.latitude+.01})).toBe(false);
});

test("at exact crossing the unoccupied alignment is offered, but nearby lines are not silently selected",()=>{
 const crossing={id:"cross",points:[{...point,latitude:point.latitude-.02},{...point,latitude:point.latitude+.02}]};
 const occupied=[{lineId:line.id,position:.5,lengthM:200}];
 expect(stationTarget([line,crossing],point,200,DEFAULT_CAMERA,1,occupied).target?.line.id).toBe("cross");
 const parallel={...line,id:"parallel",points:line.points.map(p=>({...p,latitude:p.latitude+.001}))};
 expect(stationTarget([line,parallel],point,200,DEFAULT_CAMERA,1,occupied).error).toContain("重な");
});
