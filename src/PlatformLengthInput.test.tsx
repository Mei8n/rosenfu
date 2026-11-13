import { expect,test } from "vitest";
import { convertPlatformLength } from "./PlatformLengthInput";
test("20m conversion keeps non-integer platform dimensions exact",()=>{
 for(const metres of ["20","45","231.5","999.99999","1000"]){expect(convertPlatformLength(convertPlatformLength(metres,true),false)).toBe(metres);}
 expect(convertPlatformLength("12.125",false)).toBe("242.5");expect(convertPlatformLength("2e2",true)).toBe("10");
 expect(convertPlatformLength("",true)).toBe("");expect(convertPlatformLength("NaN",true)).toBe("");
});
