import {expect,test} from "vitest";
import {IDBFactory} from "fake-indexeddb";
import {createReferenceStationStorage} from "./referenceStationStorage";
const json=JSON.stringify({format:"rosenfu-reference-stations",version:1,dataset:{id:"saved",revision:"1",source:"test",license:"test",bounds:{west:139,east:140,south:35,north:36}},stations:[]});
test("IndexedDB restores across repository instances and invalid replacement preserves prior data",async()=>{
 const idb=new IDBFactory();const store=createReferenceStationStorage(()=>idb);expect(await store.load()).toBeNull();await store.save(json);
 expect(await createReferenceStationStorage(()=>idb).load()).toBe(json);await expect(store.save('{"version":999}')).rejects.toThrow();expect(await store.load()).toBe(json);
 await expect(store.save(" ".repeat(5*1024*1024+1))).rejects.toThrow(/5MiB/);expect(await store.load()).toBe(json);
 await store.remove();expect(await store.load()).toBeNull();
});
test("serialized writes cannot resurrect data after queued removal",async()=>{
 const store=createReferenceStationStorage(()=>new IDBFactory()); // use one stable factory for ordering below
 const idb=new IDBFactory(),ordered=createReferenceStationStorage(()=>idb);
 await Promise.all([ordered.save(json),ordered.remove()]);expect(await ordered.load()).toBeNull();
 await Promise.all([ordered.remove(),ordered.save(json)]);expect(await ordered.load()).toBe(json);
 expect(await store.load()).toBeNull();
});
test("storage denial is explicit and does not poison subsequent operations",async()=>{
 const idb=new IDBFactory();let denied=true;const store=createReferenceStationStorage(()=>{if(denied)throw new DOMException("denied","SecurityError");return idb;});
 await expect(store.save(json)).rejects.toThrow(/denied/);denied=false;await store.save(json);expect(await store.load()).toBe(json);
 await expect(createReferenceStationStorage(()=>undefined).load()).rejects.toThrow(/IndexedDB/);
});
