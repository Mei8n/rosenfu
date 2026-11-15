import type {WaterEvidenceComposite} from './waterEvidenceComposite';
export const WATER_DIAGNOSTIC_TAGS=['width','tunnel','covered','intermittent','layer'] as const;
type ClaimTag=typeof WATER_DIAGNOSTIC_TAGS[number];
export type WaterEvidenceDiagnostics={
 scope:'selected-query-window';status:'incomplete';coverageCertified:false;constructionAssessment:'unavailable';
 hasMixedRevisionClaims:boolean;
 parts:{key:string;revision:string;waterAreaRecords:number;waterwayLineRecords:number;coastlineRecords:number;
  waterwayLinesWithoutWidthTag:number;waterwayLinesWithUnverifiedWidthTag:number;
  lineTagClaims:{tag:ClaimTag;value:string;records:number}[];
  sourceIssues:{id:string;reason:string}[];sourceIssueLocations:'unspecified';unreportedGapsPossible:true;
 }[];
 messages:string[];
};
/** Display diagnostics for reader-validated evidence only. No widths, surface
 * footprints, intersections, dry land or construction permissions are inferred. */
export function diagnoseWaterEvidence(evidence:WaterEvidenceComposite):WaterEvidenceDiagnostics{
 if(evidence.status!=='incomplete'||evidence.coverage.length)throw Error('Inspection diagnostics require uncertified evidence');
 const parts=evidence.parts.map(({key,pack})=>{
  if(pack.status!=='incomplete'||pack.coverage.length)throw Error('Inspection diagnostics require uncertified parts');
  const counts={'water-area':0,'waterway-line':0,coastline:0};let missingWidth=0,presentWidth=0;
  const claims=new Map<ClaimTag,Map<string,number>>(WATER_DIAGNOSTIC_TAGS.map(tag=>[tag,new Map()]));
  for(const observation of pack.observations){
   counts[observation.kind]++;
   if(observation.kind!=='waterway-line')continue;
   if(observation.tags.width?.trim())presentWidth++;else missingWidth++;
   for(const tag of WATER_DIAGNOSTIC_TAGS){const value=observation.tags[tag];if(value?.trim()){const values=claims.get(tag)!;values.set(value,(values.get(value)??0)+1);}}
  }
  return {key,revision:pack.revision,waterAreaRecords:counts['water-area'],waterwayLineRecords:counts['waterway-line'],coastlineRecords:counts.coastline,
   waterwayLinesWithoutWidthTag:missingWidth,waterwayLinesWithUnverifiedWidthTag:presentWidth,
   lineTagClaims:WATER_DIAGNOSTIC_TAGS.flatMap(tag=>[...claims.get(tag)!].map(([value,records])=>({tag,value,records}))),
   sourceIssues:pack.sourceIssues.map(issue=>({...issue})),sourceIssueLocations:'unspecified' as const,unreportedGapsPossible:true as const};
 });
 const missing=parts.reduce((n,p)=>n+p.waterwayLinesWithoutWidthTag,0),present=parts.reduce((n,p)=>n+p.waterwayLinesWithUnverifiedWidthTag,0);
 const issueCount=parts.reduce((n,p)=>n+p.sourceIssues.length,0),mixed=new Set(parts.map(p=>p.revision)).size>1;
 const messages=[
  '選択した取得範囲の検査資料です。全国の網羅性・陸地・着工可否は認定しません。',
  '件数は原典別の観測レコード数です。地域間の同一地物や重複形状を含む可能性があります。',
  `幅タグ未記入（空値・空白を含む）の水路等の線レコード ${missing}件、幅タグあり（未検証）${present}件。幅・水面形状・高さは推定していません。`,
  issueCount?`原典の問題報告 ${issueCount}件。位置は特定されておらず、表示範囲内の欠落件数を意味しません。`:'原典の問題報告は0件ですが、欠落がないことを意味しません。',
  '組立時に通知されないrelationの欠落や、原典に未収録の水域は検出できません。面や線が0件でも陸地とは判定しません。',
  mixed?'原典の版表示が異なります。同時点の水域としては扱えません。':'原典の版表示が一致しても、観測時点や完全性が保証されたわけではありません。',
 ];
 return {scope:'selected-query-window',status:'incomplete',coverageCertified:false,constructionAssessment:'unavailable',hasMixedRevisionClaims:mixed,parts,messages};
}
