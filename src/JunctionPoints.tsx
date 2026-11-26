import {useMemo} from 'react';
import {MAP_VIEWBOX,project,type Camera} from './map';
import {firstByKey} from './mapLookupIndex';
import {createTrackViewportIndex,trackViewport} from './trackViewport';
import type {GeoPoint} from './alignment';

type Endpoint={alignment_id:string;endpoint:'start'|'end'};
type Line={id:string;name?:string;points:GeoPoint[]};
type Link={a:Endpoint;b:Endpoint};
const key=(p:Endpoint)=>p.alignment_id+':'+p.endpoint;
export function JunctionPoints({lines,connections,camera,mapSize=MAP_VIEWBOX,selectable,selected,onSelectionChange,isPointerSelectionSuppressed}:{lines:Line[];connections:Link[];camera:Camera;mapSize?:{width:number;height:number};selectable:boolean;selected:string|null;onSelectionChange:(id:string|null)=>void;isPointerSelectionSuppressed?:()=>boolean}){
 const junctions=useMemo(()=>{
  if(!connections.length)return null;
  const graph=new Map<string,Set<string>>(),ends=new Map<string,Endpoint>();
  const byId=firstByKey(lines,line=>line.id);
  for(const link of connections)for(const [a,b] of [[link.a,link.b],[link.b,link.a]]){
   const id=key(a);ends.set(id,a);let edges=graph.get(id);
   if(!edges){edges=new Set();graph.set(id,edges);}edges.add(key(b));
  }
  const seen=new Set<string>(),points:{id:string;point:GeoPoint;names:string[]}[]=[];
  for(const id of graph.keys()){
   if(seen.has(id))continue;
   const stack=[id],group:string[]=[];
   while(stack.length){
    const node=stack.pop()!;
    if(seen.has(node))continue;
    seen.add(node);group.push(node);
    for(const neighbor of graph.get(node)??[])if(!seen.has(neighbor))stack.push(neighbor);
   }
   if(group.length!==3)continue;
   const endpoint=ends.get(id)!,line=byId.get(endpoint.alignment_id),point=endpoint.endpoint==='start'?line?.points[0]:line?.points.at(-1);
   const names=[...new Set(group.map(join=>byId.get(ends.get(join)!.alignment_id)?.name?.trim()).filter((name):name is string=>!!name))];
   if(point)points.push({id:group.sort().join('|'),point,names});
  }
  return createTrackViewportIndex(points.map(item=>({...item,points:[item.point]})),'display');
 },[lines,connections]);
 const visible=useMemo(()=>junctions?.visible(trackViewport(camera,mapSize))??[],[junctions,mapSize,camera]);
 return <g className="junction-points">{visible.map(item=>{
  const p=project(item.point.latitude,item.point.longitude,camera),active=selectable&&selected===item.id;
  const label=item.names.join(' / ');
  const toggle=()=>onSelectionChange(active?null:item.id);
  return <g key={item.id} role={selectable?'button':undefined} tabIndex={selectable?0:undefined} aria-label={label?`分岐ポイント：${label}`:"分岐ポイント"} aria-pressed={selectable?active:undefined} pointerEvents={selectable?'all':'none'}
   onClick={event=>{if(selectable){event.stopPropagation();if(!isPointerSelectionSuppressed?.())toggle();}}}
   onKeyDown={event=>{if(selectable&&(event.key==='Enter'||event.key===' ')){event.preventDefault();toggle();}}}>
   {label&&<title>分岐ポイント: {label}</title>}
   <circle cx={p.x} cy={p.y} r={active?7:5} fill={active?'#297bc2':'#fff'} stroke="#285a67" strokeWidth="2"/>
   <path d={`M${p.x-2},${p.y+2} L${p.x},${p.y} L${p.x+2},${p.y-2} M${p.x},${p.y} L${p.x+2},${p.y+2}`} fill="none" stroke={active?'white':'#285a67'}/>
   {active&&<text x={p.x+10} y={p.y-10} fontSize="12" fill="#244f65">分岐ポイント{label?`：${label}`:''}</text>}
  </g>;
 })}</g>;
}
