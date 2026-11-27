import type {RoadSnapshot, RoadWay} from './automaticRoadData';
import type {GraphBounds} from './referenceGraph';
import type {GeoPoint} from './alignment';
import {ROAD_PACK_RELEASE} from './roadPackRelease';

export type SavedRoadSnapshot={revision:string;source:string;source_url:string;source_sha256:string;attribution:string;license:string;coverage_verified:boolean;road_coverage:GraphBounds[];roads:RoadWay[]};
export type RoadAssessment={crossings:{road_id:string;point:GeoPoint;position:number}[];roads_ready:boolean;unknown_road_height:boolean};
export type RoadAssessments=[string,RoadAssessment][];
const contains=(outer:GraphBounds,inner:GraphBounds)=>outer.west<=inner.west&&outer.south<=inner.south&&outer.east>=inner.east&&outer.north>=inner.north;
const TOKYO_SOURCE='https://download.geofabrik.de/asia/japan/kanto-261114.osm.pbf';
const TOKYO_SHA='29967275fa5925a5dc224708e41869800471745d53db004b25104c6eddca52dc';
const TOKYO_MANIFEST='ac62a1f9ac146830434c9266ba8601bcfa3a06e270a50d59c2413f1a517e6d3d';
/** Fetch the construction footprint, independently of the road-following toggle.
 * A long plan stays unknown rather than silently truncating the requested region. */
export function constructionRoadBounds(paths:readonly {line:GeoPoint[]}[],fallback:GraphBounds):GraphBounds {
 const points=paths.flatMap(path=>path.line);
 if(!points.length)return fallback;
 let west=Infinity,south=Infinity,east=-Infinity,north=-Infinity;
 for(const p of points){west=Math.min(west,p.longitude);east=Math.max(east,p.longitude);south=Math.min(south,p.latitude);north=Math.max(north,p.latitude);}
 return {west:west-.00001,south:south-.00001,east:east+.00001,north:north+.00001};
}
/** Only the authenticated, reviewed Tokyo 36-cell release certifies extraction.
 * Neither an arbitrary polygon nor successful Overpass transport proves coverage. */
export function savedRoadSnapshot(snapshot:RoadSnapshot):SavedRoadSnapshot {
 const {source,sourceSha256}=snapshot.bundle;
 const knownTokyo=ROAD_PACK_RELEASE?.manifestSha256===TOKYO_MANIFEST&&source.source===TOKYO_SOURCE&&sourceSha256===TOKYO_SHA&&source.sourceDate==='2026-11-14T21:20:41Z'&&contains({west:139.68,south:35.6,east:139.8,north:35.72},snapshot.bounds);
 return {revision:source.sourceDate??(snapshot.bundle.graph.dataset.id.startsWith('ofm-tiles/')?snapshot.bundle.graph.dataset.revision:''),source:source.source,source_url:source.source,source_sha256:sourceSha256,attribution:source.attribution,license:source.license,coverage_verified:snapshot.coverageVerified??knownTokyo,road_coverage:[{...snapshot.bounds}],roads:snapshot.roads};
}
const sameVersion=(a:SavedRoadSnapshot,b:SavedRoadSnapshot)=>a.revision===b.revision&&a.source===b.source&&a.source_url===b.source_url&&a.source_sha256===b.source_sha256&&a.attribution===b.attribution&&a.license===b.license;
const shape=(r:RoadWay)=>JSON.stringify([r.points,r.layer,r.level,r.bridge,r.tunnel,r.classification?[r.classification.class,r.classification.subclass,r.classification.access,r.classification.oneway??null]:null]);
/** Advisory snapshots must never turn optional road loading into a construction
 * blocker. Keep old facts on conflict; avoid repeated copies and reserve save room. */
function singleRoadAttachment(snapshot:RoadSnapshot|undefined,world:{road_snapshots?:SavedRoadSnapshot[]}|null):{roads:SavedRoadSnapshot[];reason:string} {
 if(!snapshot||(!snapshot.bundle.source.sourceDate&&!snapshot.bundle.graph.dataset.id.startsWith('ofm-tiles/')))return {roads:[],reason:'道路未確認：必要範囲の参照データ未取得（通常建設は可能）'};
 const incoming=savedRoadSnapshot(snapshot),existing=world?.road_snapshots??[];
 const ofm=(source:string)=>{const m=source.match(/^https:\/\/tiles\.openfreemap\.org\/planet\/([A-Za-z0-9_]{1,64})\/14\/(0|[1-9]\d{0,4})\/(0|[1-9]\d{0,4})\.pbf$/);return m&&Number(m[2])<16384&&Number(m[3])<16384?m[1]:undefined;};
 const incomingTileVersion=ofm(incoming.source_url);
 if(incoming.source_url.startsWith('https://tiles.openfreemap.org/')&&!incomingTileVersion)return {roads:[],reason:'道路未確認：タイル出典が不正です'};
 if(incomingTileVersion&&(incomingTileVersion!==incoming.revision||existing.some(s=>ofm(s.source_url)&&ofm(s.source_url)!==incomingTileVersion)))return {roads:[],reason:'道路未確認：保存済みタイルと提供版が異なるため、以前の道路参照を保持'};
 if(existing.some(s=>s.source===incoming.source&&!sameVersion(s,incoming)))return {roads:[],reason:'道路未確認：保存済み出典と版が異なるため、既存データを保持'};
 if(existing.some(s=>sameVersion(s,incoming)&&(!incoming.coverage_verified||s.coverage_verified)&&s.road_coverage.some(b=>contains(b,snapshot.bounds))))return {roads:[],reason:'保存済み道路参照を使用（実際の交差・高さは建設後に再判定）'};
 const prior=new Map(existing.flatMap(s=>s.roads.map(r=>[r.id,r] as const)));
 if(incoming.roads.some(r=>prior.has(r.id)&&shape(prior.get(r.id)!)!==shape(r)))return {roads:[],reason:'道路未確認：保存済み道路形状と矛盾するため、新規参照の追加を見送り'};
 if(incoming.roads.some(r=>[r.layer,r.level,r.bridge,r.tunnel].some(t=>t!==null&&t.length>100)||r.classification&&[r.classification.class,r.classification.subclass,r.classification.access,r.classification.oneway??null].some((t,i)=>t===null?i===0:typeof t!=='string'||!t.trim()||new TextEncoder().encode(t).length>100||/[\p{Cc}]/u.test(t))))return {roads:[],reason:'道路未確認：未対応の道路属性（通常建設は可能）'};
 if(existing.length>=512||new TextEncoder().encode(JSON.stringify({...world,road_snapshots:[...existing,incoming]})).length>15*1024*1024)return {roads:[],reason:'道路未確認：保存容量を保護するため、新規参照の追加を見送り'};
 return {roads:[incoming],reason:incoming.coverage_verified?'取得済み範囲で道路交差を確認可能。実際の交差・高さは建設後に判定':'道路未確認：道路形状は取得済みだが、出典の範囲保証は未検証'};
}

/** Tile provenance is preserved separately: a changed viewport must not turn
 * previously saved tiles into a conflicting synthetic aggregate source. */
export function constructionRoadAttachment(snapshot:RoadSnapshot|undefined,world:{road_snapshots?:SavedRoadSnapshot[]}|null):{roads:SavedRoadSnapshot[];reason:string} {
 if(!snapshot?.tileSnapshots)return singleRoadAttachment(snapshot,world);
 if(snapshot.tileSnapshots.length===0||snapshot.tileSnapshots.length>64||snapshot.tileSnapshots.some(tile=>tile.tileSnapshots!==undefined))return {roads:[],reason:'道路未確認：タイル参照構成が不正です'};
 const roads:SavedRoadSnapshot[]=[],reasons=new Set<string>();
 for(const tile of snapshot.tileSnapshots){
  const result=singleRoadAttachment({...tile,coverageVerified:false},{...world,road_snapshots:[...(world?.road_snapshots??[]),...roads]});
  roads.push(...result.roads);reasons.add(result.reason);
 }
 return {roads,reason:[...reasons].join(' / ')};
}
