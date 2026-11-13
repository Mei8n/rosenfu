import {expect,test} from "vitest";
import {stationTarget} from "./stationTarget";
import {DEFAULT_CAMERA} from "./map";
const point={latitude:35.67504,longitude:139.75};
const line={id:"built",points:[{...point,longitude:139.73},{...point,longitude:139.77}]};
test("nearest candidate changes automatically between built and planned; invalid nearest never silently jumps",()=>{
 const planned={id:"plan",planId:1,points:line.points.map(p=>({...p,latitude:p.latitude+.001}))};
 expect(stationTarget([line,planned],point,200,DEFAULT_CAMERA,1).target?.line.id).toBe("built");
 expect(stationTarget([line,planned],{...point,latitude:point.latitude+.001},200,DEFAULT_CAMERA,1).target?.line.id).toBe("plan");
 expect(stationTarget([line,{...planned,invalid:true}],{...point,latitude:point.latitude+.001},200,DEFAULT_CAMERA,1).error).toContain("敷設できません");
});
test("end platform clamps center, short line rejects and distant cursor has no target",()=>{
 const target=stationTarget([line],line.points[0],200,DEFAULT_CAMERA,1);
 expect(target.target!.position).toBeGreaterThan(0);expect(target.target!.polygon).toHaveLength(4);
 expect(stationTarget([line],point,10000,DEFAULT_CAMERA,1).error).toContain("収まる");
 expect(stationTarget([line],{...point,latitude:36},200,DEFAULT_CAMERA,1).target).toBeNull();
});
