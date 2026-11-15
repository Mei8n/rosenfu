import {useState} from 'react';
import {project,type Camera} from './map';
import type {GeoPoint} from './alignment';
type Endpoint={alignment_id:string;endpoint:'start'|'end'};
export function JunctionPoints({lines,connections,camera,selectable}:{lines:{id:string;points:GeoPoint[]}[];connections:{a:Endpoint;b:Endpoint}[];camera:Camera;selectable:boolean}){
 const [selected,setSelected]=useState<string|null>(null);const key=(p:Endpoint)=>p.alignment_id+':'+p.endpoint;
 const graph=new Map<string,Set<string>>(),ends=new Map<string,Endpoint>();
 for(const link of connections){for(const [a,b] of [[link.a,link.b],[link.b,link.a]]){ends.set(key(a),a);if(!graph.has(key(a)))graph.set(key(a),new Set());graph.get(key(a))!.add(key(b));}}
 const seen=new Set<string>();const points:{id:string;point:GeoPoint}[]=[];
 for(const id of graph.keys()){if(seen.has(id))continue;const stack=[id],group:string[]=[];while(stack.length){const node=stack.pop()!;if(seen.has(node))continue;seen.add(node);group.push(node);stack.push(...graph.get(node)??[]);}if(group.length!==3)continue;
  const endpoint=ends.get(id)!,line=lines.find(line=>line.id===endpoint.alignment_id),point=endpoint.endpoint==='start'?line?.points[0]:line?.points.at(-1);if(point)points.push({id:group.sort().join('|'),point});
 }
 return <g className="junction-points">{points.map(item=>{const p=project(item.point.latitude,item.point.longitude,camera),active=selected===item.id;return <g key={item.id} role={selectable?'button':undefined} tabIndex={selectable?0:undefined} aria-label="分岐ポイント" aria-pressed={selectable?active:undefined} pointerEvents={selectable?'all':'none'} onClick={event=>{if(selectable){event.stopPropagation();setSelected(active?null:item.id);}}} onKeyDown={event=>{if(selectable&&(event.key==='Enter'||event.key===' ')){event.preventDefault();setSelected(active?null:item.id);}}}><circle cx={p.x} cy={p.y} r={active?7:5} fill={active?'#297bc2':'#fff'} stroke="#285a67" strokeWidth="2"/><path d={`M${p.x-2},${p.y+2} L${p.x},${p.y} L${p.x+2},${p.y-2} M${p.x},${p.y} L${p.x+2},${p.y+2}`} fill="none" stroke={active?'white':'#285a67'}/>{active&&<text x={p.x+10} y={p.y-10} fontSize="12" fill="#244f65">分岐ポイント</text>}</g>;})}</g>;
}
