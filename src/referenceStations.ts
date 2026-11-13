export const REFERENCE_MAX_BYTES=5*1024*1024;
export const REFERENCE_MAX_STATIONS=20000;
export const REFERENCE_VIEW_LIMIT=300;
export type ReferenceBounds={west:number;east:number;south:number;north:number};
export type ReferenceStation={osmType:"node"|"way"|"relation";osmId:string;name:string;longitude:number;latitude:number;kind:"station"|"halt"|"tram_stop"};
export type ReferencePack={format:"rosenfu-reference-stations";version:1;dataset:{id:string;revision:string;source:string;license:string;bounds:ReferenceBounds};stations:ReferenceStation[]};
const object=(value:unknown):Record<string,unknown>=>{if(!value||typeof value!=="object"||Array.isArray(value))throw Error("オブジェクト形式が不正です。");return value as Record<string,unknown>;};
function text(value:unknown,label:string,max=200):string {if(typeof value!=="string"||!value.trim()||value.length>max)throw Error(label+"が空または長すぎます。");return value;}
function coordinate(value:unknown,min:number,max:number):number {if(typeof value!=="number"||!Number.isFinite(value)||value<min||value>max)throw Error("座標範囲が不正です。");return value;}
export function parseReferencePack(json:string):ReferencePack {
 if(new TextEncoder().encode(json).byteLength>REFERENCE_MAX_BYTES)throw Error("5MiB以下のファイルを選択してください。");
 const root=object(JSON.parse(json));if(root.format!=="rosenfu-reference-stations"||root.version!==1)throw Error("未対応の参照駅パック形式・版です。");
 const d=object(root.dataset),b=object(d.bounds);
 const bounds={west:coordinate(b.west,-180,180),east:coordinate(b.east,-180,180),south:coordinate(b.south,-85.05112878,85.05112878),north:coordinate(b.north,-85.05112878,85.05112878)};
 if(bounds.west>=bounds.east||bounds.south>=bounds.north)throw Error("範囲の順序が不正です（日付変更線をまたぐパックは未対応）。");
 const dataset={id:text(d.id,"データセットID"),revision:text(d.revision,"版"),source:text(d.source,"出典",1000),license:text(d.license,"ライセンス",1000),bounds};
 if(!Array.isArray(root.stations)||root.stations.length>REFERENCE_MAX_STATIONS)throw Error("駅一覧は2万件以下にしてください。");
 const keys=new Set<string>();const stations=root.stations.map(raw=>{
  const station=object(raw);const {osmType,osmId,kind}=station;
  if(osmType!=="node"&&osmType!=="way"&&osmType!=="relation")throw Error("OSM要素種別が不正です。");
  if(typeof osmId!=="string"||! /^[1-9]\d{0,19}$/.test(osmId))throw Error("OSM IDは正の整数を文字列で指定してください。");
  const key=osmType+"/"+osmId;if(keys.has(key))throw Error("OSM要素IDが重複しています: "+key);keys.add(key);
  if(kind!=="station"&&kind!=="halt"&&kind!=="tram_stop")throw Error("駅種別が不正です。");
  const longitude=coordinate(station.longitude,bounds.west,bounds.east),latitude=coordinate(station.latitude,bounds.south,bounds.north);
  return {osmType,osmId,kind,name:text(station.name,"駅名"),longitude,latitude} as ReferenceStation;
 });
 return {format:"rosenfu-reference-stations",version:1,dataset,stations};
}
/** Display-only repository boundary; a later SQLite implementation can provide the same query. */
export type ReferenceNameCandidate={name:string;distanceM:number;station:ReferenceStation};
export type ReferenceNameResult={status:"outside"|"available"|"none";candidates:ReferenceNameCandidate[]};
export const REFERENCE_NAME_RADIUS_M=1000;
export interface ReferenceStationRepository { suggestNames(point:{latitude:number;longitude:number}):ReferenceNameResult;query(bounds:ReferenceBounds,limit?:number):{stations:ReferenceStation[];total:number}; }
export function createReferenceRepository(pack:ReferencePack):ReferenceStationRepository {
 return {suggestNames(point){return nearbyReferenceNames(pack,point);},query(bounds,limit=REFERENCE_VIEW_LIMIT){const matches=pack.stations.filter(s=>s.longitude>=bounds.west&&s.longitude<=bounds.east&&s.latitude>=bounds.south&&s.latitude<=bounds.north);return {stations:matches.slice(0,Math.max(0,Math.min(REFERENCE_VIEW_LIMIT,limit))),total:matches.length};}};
}

/** Display suggestions only. No selection, mutation, lookup or geocoding request occurs here. */
export function nearbyReferenceNames(pack:ReferencePack,point:{latitude:number;longitude:number}):ReferenceNameResult {
 const b=pack.dataset.bounds;
 if(!Number.isFinite(point.latitude)||!Number.isFinite(point.longitude)||point.latitude<b.south||point.latitude>b.north||point.longitude<b.west||point.longitude>b.east)return {status:"outside",candidates:[]};
 const rad=Math.PI/180,unknown=new Set(["unknown","unnamed","不明","名称不明","名前不明","駅名不明","無名","無名駅","unknown station","unnamed station","n/a","未設定","名称未設定"]);
 const usable=pack.stations.flatMap(station=>{
  const name=station.name.trim(),normalized=name.normalize("NFKC").toLowerCase();
  if(!name||name.length>80||unknown.has(normalized)||/[\p{Cc}\p{Cf}\u2028\u2029<>]/u.test(name+normalized))return [];
  const a=Math.sin((station.latitude-point.latitude)*rad/2)**2+Math.cos(point.latitude*rad)*Math.cos(station.latitude*rad)*Math.sin((station.longitude-point.longitude)*rad/2)**2;
  const distanceM=6371000*2*Math.asin(Math.sqrt(Math.max(0,Math.min(1,a))));
  return distanceM<=REFERENCE_NAME_RADIUS_M?[{name,distanceM,station}]:[];
 }).sort((a,b)=>a.distanceM-b.distanceM || (a.station.osmType+"/"+a.station.osmId).localeCompare(b.station.osmType+"/"+b.station.osmId));
 const names=new Set<string>(),candidates:ReferenceNameCandidate[]=[];
 for(const candidate of usable){const key=candidate.name.normalize("NFKC").toLowerCase();if(names.has(key))continue;names.add(key);candidates.push(candidate);if(candidates.length===5)break;}
 return {status:candidates.length?"available":"none",candidates};
}
