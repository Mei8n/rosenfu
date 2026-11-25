import {useMemo,useState} from 'react';
import {firstByKey} from './mapLookupIndex';
import {ServiceBadge,initialServiceSymbol,type ServiceSymbolShape} from './ServiceAppearanceControls';
import './directoryPanel.css';
import type {ServiceRouteBindingRecord} from './ServiceRouteBinding';
import {useRailServiceActivity} from './OperationsPanel';
type Service={id:string;name:string};
type Appearance={service_id:string;color:string;shape?:ServiceSymbolShape;symbol?:string|null};
const NO_APPEARANCES:Appearance[]=[];
type Route={id:string;name:string};
const NO_ROUTES:Route[]=[];
const NO_BINDINGS:ServiceRouteBindingRecord[]=[];
const ALL='all';
const INDEPENDENT='independent';
const normalize=(text:string)=>text.normalize('NFKC').toLocaleLowerCase('ja');
export function ServiceDirectory({services,appearances=NO_APPEARANCES,routes=NO_ROUTES,bindings=NO_BINDINGS,selected,enabled,onChoose,onCreate,showLiveOperation=true}:{services:Service[];appearances?:Appearance[];routes?:Route[];bindings?:ServiceRouteBindingRecord[];selected:string|null;enabled:boolean;onChoose:(id:string|null)=>void;onCreate:()=>void;showLiveOperation?:boolean}){
 const activity=useRailServiceActivity(showLiveOperation);
 const [query,setQuery]=useState('');
 const [requestedFilter,setFilter]=useState(ALL);
 const [grouped,setGrouped]=useState(false);
 const normalized=normalize(query.trim());
 const routesById=useMemo(()=>firstByKey(routes,route=>route.id),[routes]);
 const routesByService=useMemo(()=>{
  const known=firstByKey(bindings,record=>record.service_id);
  const result=new Map<string,Route[]>();
  for(const service of services){
   const binding=known.get(service.id)?.binding;
   const ids=binding?.kind==='route'?[binding.route_id]:binding?.kind==='routes'?binding.route_ids:[];
   result.set(service.id,[...new Set(ids)].flatMap(id=>{const route=routesById.get(id);return route?[route]:[];}));
  }
  return result;
 },[services,bindings,routesById]);
 const usedRoutes=useMemo(()=>{
  const used=new Set<string>();
  for(const serviceRoutes of routesByService.values())for(const route of serviceRoutes)used.add(route.id);
  return routes.filter(route=>used.has(route.id));
 },[routes,routesByService]);
 const filter=requestedFilter===INDEPENDENT&&usedRoutes.length>0||usedRoutes.some(route=>'route:'+route.id===requestedFilter)?requestedFilter:ALL;
 const visible=useMemo(()=>services.filter(service=>{
  const membership=routesByService.get(service.id)??NO_ROUTES;
  return (filter===ALL||(filter===INDEPENDENT?!membership.length:membership.some(route=>'route:'+route.id===filter)))&&
   (!normalized||normalize(service.name).includes(normalized)||membership.some(route=>normalize(route.name).includes(normalized)));
 }),[services,routesByService,filter,normalized]);
 const appearancesByService=useMemo(()=>firstByKey(appearances,row=>row.service_id),[appearances]);
 const groups=useMemo(()=>{
  if(!grouped||usedRoutes.length===0)return [{id:'all',name:null,rows:visible}];
  const singles=new Map<string,Service[]>(),through:Service[]=[],independent:Service[]=[];
  for(const row of visible){
   const member=routesByService.get(row.id)??NO_ROUTES;
   if(member.length>1)through.push(row);
   else if(!member.length)independent.push(row);
   else{
    const rows=singles.get(member[0].id);
    if(rows)rows.push(row);else singles.set(member[0].id,[row]);
   }
  }
  return [
   ...usedRoutes.flatMap(route=>{const rows=singles.get(route.id);return rows?[{id:'route:'+route.id,name:route.name,rows}]:[];}),
   ...(through.length?[{id:'through',name:'複数の路線にまたがる系統',rows:through}]:[]),
   ...(independent.length?[{id:'independent',name:'独立した系統',rows:independent}]:[]),
  ];
 },[grouped,usedRoutes,visible,routesByService]);
 return <div className="service-directory network-directory">
  <div className="directory-toolbar"><span className="directory-total">{services.length}系統</span><button type="button" className="secondary" disabled={!enabled} onClick={onCreate}>新規系統</button></div>
  <label className="directory-search">系統を探す<input type="search" placeholder={usedRoutes.length?"系統名・路線名":"系統名"} value={query} onChange={event=>setQuery(event.target.value)}/></label>
  {usedRoutes.length>0&&<label className="directory-search">路線で絞り込む<select value={filter} onChange={event=>setFilter(event.target.value)}>
   <option value={ALL}>すべての系統</option>{usedRoutes.map(route=><option key={route.id} value={'route:'+route.id}>{route.name}</option>)}<option value={INDEPENDENT}>独立した系統</option>
  </select></label>}
  {usedRoutes.length>0&&<label className="service-directory-group-toggle"><input type="checkbox" checked={grouped} onChange={event=>setGrouped(event.target.checked)}/>所属路線ごとに表示</label>}
  <table aria-label="系統一覧"><thead><tr><th>名前</th><th>乗客</th><th>収支</th><th>稼働中</th></tr></thead>{groups.map(group=><tbody key={group.id}>{group.name&&<tr className="service-directory-group"><th scope="rowgroup" colSpan={4}>{group.name} <small>{group.rows.length}系統</small></th></tr>}{group.rows.map(service=>{const appearance=appearancesByService.get(service.id), active=activity?.get(service.id), membership=routesByService.get(service.id)??NO_ROUTES;return <tr key={service.id}><th><button type="button" className={`route-choice ${service.id===selected?'selected':''}`} disabled={!enabled} onClick={()=>onChoose(service.id===selected?null:service.id)} aria-label={`系統編集: ${service.name}`} aria-pressed={service.id===selected}><ServiceBadge color={appearance?.color??'#3b82f6'} shape={appearance?.shape??'circle'} symbol={appearance?.symbol??initialServiceSymbol(service.name)}/><span>{service.name}{membership.length>0&&<small className="service-directory-routes">{membership.map(route=>route.name).join(" / ")}</small>}</span></button></th>{['乗客','収支'].map(label=><td key={label} aria-label={`${label}: 未実装`}>—</td>)}<td aria-label={active!==undefined?`稼働中: ${active}編成`:activity?'稼働中: 運行未設定':'稼働中: 情報なし'}>{active!==undefined?`${active}編成`:'—'}</td></tr>;})}</tbody>)}</table>
  {!visible.length&&<p className="empty-state">{services.length?'一致する系統がありません':'まだ系統はありません。新規系統から地図上の駅を選択してください。'}</p>}
  {visible.length>0&&<small className="directory-note">稼働中は走行・停車・進路待ちの編成数です。保有編成数ではありません。乗客・収支は未対応です。</small>}
 </div>;
}
