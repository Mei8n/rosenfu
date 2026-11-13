import { parseReferencePack } from "./referenceStations";
export interface ReferenceStationStorage { load():Promise<string|null>; save(json:string):Promise<void>; remove():Promise<void>; }
/** Single-key transactional cache, isolated from World saves and other app databases. */
export function createReferenceStationStorage(factory:()=>IDBFactory|undefined=()=>globalThis.indexedDB):ReferenceStationStorage {
 let queue:Promise<unknown>=Promise.resolve();
 function serialize<T>(task:()=>Promise<T>):Promise<T>{const next=queue.then(task,task);queue=next.catch(()=>{});return next;}
 function operation(mode:IDBTransactionMode,action:(store:IDBObjectStore)=>IDBRequest):Promise<unknown>{
  return new Promise((resolve,reject)=>{
   const idb=factory();if(!idb){reject(Error("この環境ではIndexedDBを利用できません。"));return;}
   let settled=false;const opening=idb.open("rosenfu-reference-stations",1);
   const timer=setTimeout(()=>{settled=true;reject(Error("端末保存の開始がタイムアウトしました。"));},5000);
   const fail=(error:unknown)=>{clearTimeout(timer);if(!settled){settled=true;reject(error);}};
   opening.onerror=()=>fail(opening.error??Error("端末保存を開けません。"));opening.onblocked=()=>fail(Error("別の画面が端末保存を使用中です。"));
   opening.onupgradeneeded=()=>{if(!opening.result.objectStoreNames.contains("packs"))opening.result.createObjectStore("packs");};
   opening.onsuccess=()=>{
    const db=opening.result;if(settled){db.close();return;}clearTimeout(timer);
    try {const transaction=db.transaction("packs",mode);const request=action(transaction.objectStore("packs"));let result:unknown;
     request.onsuccess=()=>{result=request.result;};
     transaction.oncomplete=()=>{db.close();if(!settled){settled=true;resolve(result);}};
     transaction.onabort=transaction.onerror=()=>{db.close();fail(transaction.error??request.error??Error("端末保存に失敗しました。"));};
    } catch(error){db.close();fail(error);}
   };
  });
 }
 return {
  load:()=>serialize(async()=>{const value=await operation("readonly",store=>store.get("current"));if(value===undefined)return null;if(typeof value!=="string")throw Error("保存済み参照パックの型が不正です。");return value;}),
  save:(json)=>serialize(async()=>{parseReferencePack(json);await operation("readwrite",store=>store.put(json,"current"));}),
  remove:()=>serialize(async()=>{await operation("readwrite",store=>store.delete("current"));})
 };
}
export const referenceStationStorage=createReferenceStationStorage();
