import type {ReferenceStationRepository} from './referenceStations';
/** Local-only provisional naming. Never assumes a nearby reference is the same station. */
export function automaticStationName(point:{latitude:number;longitude:number}|null,repository:ReferenceStationRepository|null,existingNames:readonly string[]){
 const result=point&&repository?repository.suggestNames(point):null;
 const candidate=result?.candidates[0];
 const base=candidate?.name??'新駅';
 const used=new Set(existingNames.map(name=>name.normalize('NFKC').toLowerCase()));
 let name=base,index=2;
 while(used.has(name.normalize('NFKC').toLowerCase())){const suffix=` ${index++}`;let stem=base;while(stem.length+suffix.length>80)stem=Array.from(stem).slice(0,-1).join('');name=stem+suffix;}
 return {name,candidate,status:result?.status??'missing'};
}
