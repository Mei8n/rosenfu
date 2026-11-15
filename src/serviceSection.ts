import type {Segment} from './RouteCandidatePreview';
import {corridorSpans} from './connectedCorridor';
import {resolveServicePath} from './servicePaths';
import type {PathWorld} from './servicePaths';
export function routeSection(start:string,segments:Segment[],from:number,to:number,world:PathWorld){
 const all=resolveServicePath(start,segments.map((segment,i)=>({key:String(i+1),segment})),world);
 if(!Number.isInteger(from)||!Number.isInteger(to)||from<0||to<0||from>=all.length||to>=all.length||from===to)throw Error('異なる有効な駅出現位置を選んでください');
 const low=Math.min(from,to),high=Math.max(from,to);let offset=0,current=start;const selected:Segment[]=[];
 for(const segment of segments){
  const stations=resolveServicePath(current,[{key:'1',segment}],world).map(o=>o.station_id),end=offset+stations.length-1;
  const a=Math.max(low,offset)-offset,b=Math.min(high,end)-offset;
  if(a<b){
   if(segment.kind==='alignment')selected.push({...segment,from:stations[a],to:stations[b]});
   else{
    const spans=corridorSpans(segment,world);const places=world.station_placements??[];
    const pa=places.find(p=>p.station_id===stations[a])!,pb=places.find(p=>p.station_id===stations[b])!;
    const ia=spans.findIndex(s=>s.alignmentId===pa.alignment_id&&pa.position>=Math.min(s.begin,s.finish)&&pa.position<=Math.max(s.begin,s.finish));
    const ib=spans.findIndex((s,i)=>i>=ia&&s.alignmentId===pb.alignment_id&&pb.position>=Math.min(s.begin,s.finish)&&pb.position<=Math.max(s.begin,s.finish));
    if(ia<0||ib<ia)throw Error('接続区間を切り出せません');
    selected.push(ia===ib?{kind:'alignment',alignment_id:pa.alignment_id,from:stations[a],to:stations[b]}:{...segment,from:stations[a],to:stations[b],connections:segment.connections.slice(ia,ib)});
   }
  }
  offset=end;current=stations.at(-1)!;
 }
 if(from>to){selected.reverse();for(let i=0;i<selected.length;i++){const s=selected[i];if(s.kind==='alignment')selected[i]={...s,from:s.to,to:s.from};else if(s.kind==='connected_alignments')selected[i]={...s,from:s.to,to:s.from,connections:[...s.connections].reverse().map(c=>({a:c.b,b:c.a}))};}}
 const result={start:all[from].station_id,segments:selected};
 const actual=resolveServicePath(result.start,selected.map((segment,i)=>({key:String(i+1),segment})),world).map(o=>o.station_id);
 const expected=all.slice(low,high+1).map(o=>o.station_id);if(from>to)expected.reverse();
 if(JSON.stringify(actual)!==JSON.stringify(expected))throw Error('区間の駅列が一致しません');
 return result;
}
