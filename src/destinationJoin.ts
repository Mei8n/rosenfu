import type {Extension,GeoPoint} from "./constructionPlans";
import type {TrackType} from "./TrackTypeControls";
import {extensionJoinError} from "./extensionJoin";
import {endpointInfrastructure,type SectionInfrastructure} from "./sectionInfrastructure";
export type EndpointRef={alignment_id:string;endpoint:"start"|"end"};
export type EndpointConnection={a:EndpointRef;b:EndpointRef};
type Profile={start_height_m:number;end_height_m:number;start_tangent_m?:number;end_tangent_m?:number};
export type JoinAlignment={id:string;owner:string;track_type?:TrackType;points:GeoPoint[];infrastructure?:SectionInfrastructure[];vertical_profile?:Profile|null;section_profiles?:Profile[]};
export type BranchPair={source:Extension;destination:Extension};
export const endpointKey=(ref:Extension)=>ref.alignmentId+":"+ref.endpoint;

// Offer one additional port only at an isolated two-port join. An endpoint
// that belongs to an already formed three-port junction is not a candidate,
// even if that endpoint itself has only one link.
export function branchStemEnds<T extends JoinAlignment>(alignments:T[],connections:EndpointConnection[],owner:string|undefined){
 const byId=new Map(alignments.map(alignment=>[alignment.id,alignment]));
 const neighbours=new Map<string,EndpointRef[]>();
 const add=(from:EndpointRef,to:EndpointRef)=>{
  const key=endpointKey({alignmentId:from.alignment_id,endpoint:from.endpoint});
  const list=neighbours.get(key);
  if(list)list.push(to);else neighbours.set(key,[to]);
 };
 for(const {a,b} of connections){add(a,b);add(b,a);}
 const ends=[];
 for(const [key,adjacent] of neighbours){
  if(adjacent.length!==1)continue;
  const [other]=adjacent;
  const reverse=neighbours.get(endpointKey({alignmentId:other.alignment_id,endpoint:other.endpoint}));
  if(reverse?.length!==1)continue;
  const own=reverse[0];
  const toInfo=(ref:EndpointRef)=>{
   const alignment=byId.get(ref.alignment_id);
   if(!alignment||!owner||alignment.owner!==owner||(alignment.track_type??"normal")!=="normal")return;
   const infra=endpointInfrastructure(alignment,ref.endpoint);
   if(infra.level!=="ground"||infra.track_count!==1)return;
   const profiles=alignment.section_profiles;
   const profile=(ref.endpoint==="start"?profiles?.[0]:profiles?.at(-1))??alignment.vertical_profile;
   if(profile&&(profile.start_height_m!==0||profile.end_height_m!==0||
      (profile.start_tangent_m??0)!==0||(profile.end_tangent_m??0)!==0))return;
   const point=ref.endpoint==="start"?alignment.points[0]:alignment.points.at(-1);
   if(point)return {alignment,endpoint:ref.endpoint,point,key};
  };
  const a=toInfo(own),b=toInfo(other);
  if(a&&b&&a.point.latitude===b.point.latitude&&a.point.longitude===b.point.longitude)ends.push(a);
 }
 return ends;
}

export function occupiedEndpoint(connections:EndpointConnection[],end:Extension){
 return connections.some(c=>[c.a,c.b].some(e=>e.alignment_id===end.alignmentId&&e.endpoint===end.endpoint));
}
// A mixed join is between a tram-only alignment and conventional rail.
export function mixedTramTracks(a:TrackType,b:TrackType){
 return (a==="tram")!==(b==="tram");
}
export function destinationJoinError(line:GeoPoint[],source:Extension|null,destination:Extension|undefined,alignments:JoinAlignment[],connections:EndpointConnection[],owner:string|undefined,trackType:TrackType="normal",branch?:BranchPair):string|null {
 if(!destination)return null;
 const a=alignments.find(a=>a.id===source?.alignmentId),b=alignments.find(a=>a.id===destination.alignmentId);
 if(!b||(source&&(!a||(a.id===b.id&&source.endpoint===destination.endpoint))))return "接続先の線路が無効です";
 if((a&&a.owner!==owner)||b.owner!==owner)return "他社の線路には接続できません";
 const sourceKind=a?.track_type??"normal",targetKind=b.track_type??"normal";
 if((a&&sourceKind!==trackType&&!mixedTramTracks(sourceKind,trackType))
    ||(targetKind!==trackType&&!mixedTramTracks(trackType,targetKind)))return "線路種別が一致しません";
 if((source&&occupiedEndpoint(connections,source))||occupiedEndpoint(connections,destination)){
  if(!branch||!source||trackType!=="normal"||endpointKey(source)!==endpointKey(branch.source)||
      endpointKey(destination)!==endpointKey(branch.destination))return "この端点は接続済みです";
  const ends=branchStemEnds(alignments,connections,owner);
  const left=ends.find(e=>e.key===endpointKey(source)),right=ends.find(e=>e.key===endpointKey(destination));
  if(!left||!right||left.point.latitude===right.point.latitude&&left.point.longitude===right.point.longitude)return "この端点には分岐を追加できません";
 }
 const end=destination.endpoint==="start"?b.points[0]:b.points.at(-1);
 if(line.length<2||!end||line.at(-1)!.latitude!==end.latitude||line.at(-1)!.longitude!==end.longitude)return "接続先の端点に一致していません";
 const inside=destination.endpoint==="start"?b.points[1]:b.points.at(-2);
 return extensionJoinError([...line].reverse(),inside)?"接続先の接線方向に合わせてください":null;
}
