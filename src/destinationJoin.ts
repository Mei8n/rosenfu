import type {Extension,GeoPoint} from "./constructionPlans";
import type {TrackType} from "./TrackTypeControls";
import {extensionJoinError} from "./extensionJoin";
export type EndpointRef={alignment_id:string;endpoint:"start"|"end"};
export type EndpointConnection={a:EndpointRef;b:EndpointRef};
export type JoinAlignment={id:string;owner:string;track_type?:TrackType;points:GeoPoint[]};
export function occupiedEndpoint(connections:EndpointConnection[],end:Extension){
 return connections.some(c=>[c.a,c.b].some(e=>e.alignment_id===end.alignmentId&&e.endpoint===end.endpoint));
}
export function destinationJoinError(line:GeoPoint[],source:Extension|null,destination:Extension|undefined,alignments:JoinAlignment[],connections:EndpointConnection[],owner:string|undefined):string|null {
 if(!destination)return null;
 const a=alignments.find(a=>a.id===source?.alignmentId),b=alignments.find(a=>a.id===destination.alignmentId);
 if(!source||!a||!b||a.id===b.id)return "接続先の線路が無効です";
 if(a.owner!==owner||b.owner!==owner)return "他社の線路には接続できません";
 if((a.track_type??"normal")!==(b.track_type??"normal"))return "線路種別が一致しません";
 if(occupiedEndpoint(connections,source)||occupiedEndpoint(connections,destination))return "この端点は接続済みです";
 const end=destination.endpoint==="start"?b.points[0]:b.points.at(-1);
 if(line.length<2||!end||line.at(-1)!.latitude!==end.latitude||line.at(-1)!.longitude!==end.longitude)return "接続先の端点に一致していません";
 const inside=destination.endpoint==="start"?b.points[1]:b.points.at(-2);
 return extensionJoinError([...line].reverse(),inside)?"接続先の接線方向に合わせてください":null;
}
