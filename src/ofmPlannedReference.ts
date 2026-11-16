import type {PlannedPath} from './constructionPlans';
import type {prepareSharedOfmEvidenceDraft} from './ofmSharedEvidenceDraft';
export type OfmPathDescriptor=Awaited<ReturnType<typeof prepareSharedOfmEvidenceDraft>>['persistentProposal']['alignment'];
/** Matches Rust ConstructionPath.ofm_path via camelCase. Existing reference remains legacy. */
export type OfmPlannedPath=Omit<PlannedPath,'reference'>&{reference?:never;ofmPath:OfmPathDescriptor};
export type ReferenceCapablePlannedPath=(PlannedPath&{ofmPath?:never})|OfmPlannedPath;
const error=():never=>{throw Error('OFM計画の参照形式が不正です。再取得してください。');};
function object(v:unknown,keys:string[]):Record<string,unknown>{if(!v||typeof v!=='object'||Array.isArray(v)||Object.keys(v).length!==keys.length||Object.keys(v).some(k=>!keys.includes(k)))error();return v as Record<string,unknown>;}
function finite(v:unknown,min:number,max:number):v is number{return typeof v==='number'&&Number.isFinite(v)&&v>=min&&v<=max;}
function fragment(v:unknown):v is string{return typeof v==='string'&&/^ofm:[A-Za-z0-9_]{1,64}:14:(0|[1-9]\d{0,4}):(0|[1-9]\d{0,4}):(0|[1-9]\d{0,3}):(0|[1-9]\d{0,8})$/.test(v);}
function binding(v:unknown):v is string{if(typeof v!=='string')return false;const m=v.match(/^https:\/\/tiles\.openfreemap\.org\/planet\/([A-Za-z0-9_]{1,64})\/14\/(0|[1-9]\d{0,4})\/(0|[1-9]\d{0,4})\.pbf$/);return !!m&&Number(m[2])<16384&&Number(m[3])<16384;}
/** Structural parsing only; backend must re-decode evidence and validate/reconstruct the path. */
export function parseOfmPathDescriptor(value:unknown):OfmPathDescriptor{
 const v=object(value,['method','algorithm','geometryDerived','coverageVerified','requiredBindingKeys','start','end','orderedSpans','joins']);
 if(v.method!=='ofm_tile_path_v1'||v.algorithm!=='ofm_overlap_blend_v1'||v.geometryDerived!==true||v.coverageVerified!==false)error();
 if(!Array.isArray(v.requiredBindingKeys)||!v.requiredBindingKeys.length||v.requiredBindingKeys.length>16||!v.requiredBindingKeys.every(binding)||new Set(v.requiredBindingKeys).size!==v.requiredBindingKeys.length)error();
 for(const p of [v.start,v.end]){const q=object(p,['fragmentId','segmentIndex','fraction']);if(!fragment(q.fragmentId)||!finite(q.segmentIndex,0,510)||!Number.isInteger(q.segmentIndex)||!finite(q.fraction,0,1))error();}
 if(!Array.isArray(v.orderedSpans)||v.orderedSpans.length<2||v.orderedSpans.length>32||!Array.isArray(v.joins)||v.joins.length!==v.orderedSpans.length-1)error();
 const spans=v.orderedSpans as unknown[],joins=v.joins as unknown[];let segments=0;
 for(const s of spans){const q=object(s,['bindingKey','fragmentId','from','to']);if(!binding(q.bindingKey)||!(v.requiredBindingKeys as unknown[]).includes(q.bindingKey)||!fragment(q.fragmentId)||!finite(q.from,0,511)||!finite(q.to,0,511))error();const from=q.from as number,to=q.to as number;segments+=from===to?0:Math.ceil(Math.max(from,to))-Math.floor(Math.min(from,to));}
 if(segments>256)error();
 joins.forEach((j,i)=>{const q=object(j,['rule','aId','bId','axis','fromAxis','toAxis','aRange','bRange']);if(q.rule!=='ofm_overlap_blend_v1'||q.aId!==(spans[i] as {fragmentId:string}).fragmentId||q.bId!==(spans[i+1] as {fragmentId:string}).fragmentId||(q.axis!==0&&q.axis!==1)||!finite(q.fromAxis,-180,180)||!finite(q.toAxis,-180,180))error();for(const r of [q.aRange,q.bRange])if(!Array.isArray(r)||r.length!==2||!r.every(n=>finite(n,0,511)))error();});
 const json=JSON.stringify(value);if(new TextEncoder().encode(json).length>128*1024)error();return JSON.parse(json) as OfmPathDescriptor;
}
export function hasOfmPath<T extends {ofmPath?:unknown}>(path:T):path is T&{ofmPath:unknown}{return path.ofmPath!==undefined&&path.ofmPath!==null;}
/** Return only the lightweight DTO for construction-plan JSON; never evidence/raw bytes. */
export function serializePlannedReference(path:{reference?:PlannedPath['reference'];ofmPath?:unknown;extension?:unknown;destination?:unknown}):{reference?:PlannedPath['reference'];ofmPath?:OfmPathDescriptor}{
 if(hasOfmPath(path)){
  if(path.reference!=null)throw Error('実在形状の単一区間参照とタイル境界参照は併用できません。');
  if(path.extension!=null||path.destination!=null)throw Error('タイル境界を跨ぐ道路追従の既存線路への延伸・接続はまだ未対応です。独立した線路として計画してください。');
  return {ofmPath:parseOfmPathDescriptor(path.ofmPath)};
 }
 return path.reference?{reference:path.reference}:{};
}
