import {expect,it} from "vitest";
import {firstByKey} from "./mapLookupIndex";

it("matches find semantics for duplicates, absent keys and empty sources",()=>{
  const items=[{key:"a",n:1},{key:"b",n:2},{key:"a",n:3}];
  const index=firstByKey(items,item=>item.key);
  for(const key of ["a","b","missing"]){
    expect(index.get(key)).toBe(items.find(item=>item.key===key));
  }
  expect(firstByKey([],()=>"_").size).toBe(0);
});
