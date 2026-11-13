import {expect,test} from "vitest";
import {parseReferencePack,createReferenceRepository,REFERENCE_MAX_BYTES} from "./referenceStations";
export const fixture={format:"rosenfu-reference-stations",version:1,dataset:{id:"test",revision:"1",source:"synthetic fixture",license:"test only",bounds:{west:139,east:140,south:35,north:36}},stations:[{osmType:"node",osmId:"123",name:"参考テスト",longitude:139.75,latitude:35.67504,kind:"station"}]};
test("pack parser preserves string IDs and checks all metadata and station constraints",()=>{
 expect(parseReferencePack(JSON.stringify(fixture)).stations[0].osmId).toBe("123");
 for(const changed of [{...fixture,version:2},{...fixture,dataset:{...fixture.dataset,license:""}},{...fixture,stations:[...fixture.stations,...fixture.stations]},{...fixture,stations:[{...fixture.stations[0],osmId:123}]},{...fixture,stations:[{...fixture.stations[0],latitude:37}]},{...fixture,stations:[{...fixture.stations[0],kind:"bus_stop"}]},{...fixture,stations:Array(20001).fill(fixture.stations[0])}])expect(()=>parseReferencePack(JSON.stringify(changed))).toThrow();
 expect(()=>parseReferencePack(" ".repeat(REFERENCE_MAX_BYTES+1))).toThrow(/5MiB/);
});
test("repository only returns visible stations with hard display cap",()=>{
 const pack=parseReferencePack(JSON.stringify({...fixture,stations:Array.from({length:350},(_,i)=>({...fixture.stations[0],osmId:String(i+1)}))}));const repository=createReferenceRepository(pack);
 expect(repository.query(fixture.dataset.bounds,10000).stations).toHaveLength(300);expect(repository.query(fixture.dataset.bounds).total).toBe(350);
 expect(repository.query({west:130,east:131,south:32,north:33}).total).toBe(0);
});
test("nearby name candidates are bounded, nearest-first and deduplicated without changing names",()=>{
 const pack=parseReferencePack(JSON.stringify({...fixture,stations:[
 {...fixture.stations[0],osmId:"1",name:"Central",latitude:35.67604},
 {...fixture.stations[0],osmId:"2",name:"Ｃｅｎｔｒａｌ",latitude:35.67514},
 ...Array.from({length:8},(_,index)=>({...fixture.stations[0],osmId:String(index+3),name:"候補"+index,latitude:35.67524+index*.0001})),
 {...fixture.stations[0],osmId:"20",name:"遠方",latitude:35.8}
 ]}));const result=createReferenceRepository(pack).suggestNames({latitude:35.67504,longitude:139.75});
 expect(result.status).toBe("available");expect(result.candidates).toHaveLength(5);expect(result.candidates[0].name).toBe("Ｃｅｎｔｒａｌ");expect(result.candidates[0].distanceM).toBeCloseTo(11.1195,2);expect(result.candidates.some(c=>c.name==="Central"||c.name==="遠方")).toBe(false);expect(result.candidates.map(c=>c.distanceM)).toEqual([...result.candidates.map(c=>c.distanceM)].sort((a,b)=>a-b));
 expect(createReferenceRepository(pack).suggestNames({latitude:34,longitude:139.75})).toEqual({status:"outside",candidates:[]});
});
test("unknown, control, HTML-like and overlong names are not offered",()=>{
 const names=["unknown","名称不明","A\u202eB","<script>x</script>","＜script＞","x".repeat(81)];const pack=parseReferencePack(JSON.stringify({...fixture,stations:names.map((name,index)=>({...fixture.stations[0],osmId:String(index+1),name}))}));expect(createReferenceRepository(pack).suggestNames({latitude:35.67504,longitude:139.75})).toEqual({status:"none",candidates:[]});
});
test("reference name radius includes 999m and excludes 1001m",()=>{
 const latitude=35.67504,longitude=139.75,offset=(metres:number)=>metres/6371000*180/Math.PI;
 const pack=parseReferencePack(JSON.stringify({...fixture,stations:[{...fixture.stations[0],osmId:"1",name:"範囲内",latitude:latitude+offset(999)},{...fixture.stations[0],osmId:"2",name:"範囲外",latitude:latitude+offset(1001)}]}));
 const result=createReferenceRepository(pack).suggestNames({latitude,longitude});expect(result.candidates.map(item=>item.name)).toEqual(["範囲内"]);expect(result.candidates[0].distanceM).toBeCloseTo(999,5);
});
