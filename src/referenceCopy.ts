import {extractReferenceWayPath} from './referenceGraphPath';
import type {ReferenceGraphPick} from './referenceGraphPath';
import type {ReferenceGraphBundle} from './referenceGraphBundle';
import {alignmentLength} from './alignment';
import {MAP_BOUNDS} from './map';
import type {SourceRecord,AlignmentSource} from './AlignmentProvenance';
export type ReferenceBuild={name:string;points:{latitude:number;longitude:number}[];sourceRecord:SourceRecord;provenance:AlignmentSource};
export function prepareReferenceCopy(bundle:ReferenceGraphBundle,start:ReferenceGraphPick,end:ReferenceGraphPick,name:string):ReferenceBuild{
 const path=extractReferenceWayPath(bundle.graph,start,end);const points=path.points;
 if(points.some(p=>!Number.isFinite(p.latitude)||!Number.isFinite(p.longitude)||p.latitude<MAP_BOUNDS.south||p.latitude>MAP_BOUNDS.north||p.longitude<MAP_BOUNDS.west||p.longitude>MAP_BOUNDS.east))throw Error('建設範囲外です（北緯20〜46度・東経122〜154度）。');
 if(alignmentLength(points)<20)throw Error('建設長が20m未満です。探索半径20mとは別の最短建設長です。');
 const first=bundle.graph.edges.find(e=>e.id===start.edgeId)!,last=bundle.graph.edges.find(e=>e.id===end.edgeId)!;
 const sourceRecord:SourceRecord={key:bundle.graphSha256+':'+bundle.sourceSha256,dataset_id:bundle.graph.dataset.id,revision:bundle.graph.dataset.revision,graph_sha256:bundle.graphSha256,source_sha256:bundle.sourceSha256,source:bundle.source.source,source_date:bundle.source.sourceDate,license:bundle.source.license,attribution:bundle.source.attribution,conversion_format_version:1};
 for(const [value,max] of [[sourceRecord.dataset_id,200],[sourceRecord.revision,200],[sourceRecord.source,1000],[sourceRecord.source_date??'',64]] as const)if(new TextEncoder().encode(value).byteLength>max)throw Error('出典情報がWorld保存上限を超えています。黙って短縮せずコピーを拒否しました。');
 if(!/^[a-f0-9]{64}$/.test(sourceRecord.graph_sha256)||!/^[a-f0-9]{64}$/.test(sourceRecord.source_sha256))throw Error('検証済みバンドルのハッシュがありません。再読込してください。');
 return {name:name.trim(),points,sourceRecord,provenance:{source_key:sourceRecord.key,method:'osm_way_slice_v1',osm_way_id:path.osmWayId,mode:path.mode,start_segment_index:first.segmentIndex,start_fraction:start.fraction,end_segment_index:last.segmentIndex,end_fraction:end.fraction}};
}
