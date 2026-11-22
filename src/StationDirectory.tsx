import {useMemo,useState} from 'react';
import './stationPanel.css';
import {firstByKey} from './mapLookupIndex';
type Station={id:string;name:string};
type Group={station_id:string;members:string[]};
type Route={id:string;name:string;start:string};
type Path={route_id:string;member_stations:string[]};
type Placement={station_id:string};
const NO_GROUPS:Group[]=[],NO_PATHS:Path[]=[],NO_PLACEMENTS:Placement[]=[];
/** A station group is one row; selecting an individual platform still highlights its station. */
export function StationDirectory({stations,groups=NO_GROUPS,routes,paths=NO_PATHS,placements=NO_PLACEMENTS,selected,onSelect}:{stations:Station[];groups?:Group[];routes:Route[];paths?:Path[];placements?:Placement[];selected:string|null;onSelect:(id:string)=>void}){
 const [query,setQuery]=useState('');
 const rows=useMemo(()=>{
  const groupByMember=new Map(groups.flatMap(group=>group.members.map(id=>[id,group] as const)));
  const routesByStation=new Map<string,Set<string>>();
  const pathByRoute=firstByKey(paths,path=>path.route_id),platforms=new Set(placements.map(p=>p.station_id));
  for(const route of routes){for(const id of pathByRoute.get(route.id)?.member_stations??[route.start]){const names=routesByStation.get(id)??new Set<string>();names.add(route.name);routesByStation.set(id,names);}}
  return stations.filter(station=>!groupByMember.has(station.id)||groupByMember.get(station.id)!.station_id===station.id).map(station=>{
   const members=groupByMember.get(station.id)?.members??[station.id];
   return {station,members,platformCount:members.filter(id=>platforms.has(id)).length,routeNames:[...new Set(members.flatMap(id=>[...(routesByStation.get(id)??[])]))]};
  });
 },[stations,groups,routes,paths,placements]);
 const normalized=query.trim().normalize('NFKC').toLocaleLowerCase('ja');
 const visible=useMemo(()=>normalized?rows.filter(row=>[row.station.name,...row.routeNames].some(name=>name.normalize('NFKC').toLocaleLowerCase('ja').includes(normalized))):rows,[rows,normalized]);
 return <>
  <div className="directory-heading"><h3 id="stations-title">配置済みの駅</h3><span className="count" aria-label={`${rows.length}駅`}>{rows.length}</span></div>
  <div className="station-search"><label htmlFor="station-search">駅を探す</label><input id="station-search" type="search" placeholder="駅名・路線名" value={query} onChange={event=>setQuery(event.target.value)}/></div>
  {rows.length?<><div className="station-list-heading" aria-hidden="true"><span>駅名 / 所属路線</span><span>ホーム</span></div><ul aria-label="駅一覧">{visible.map(({station,members,platformCount,routeNames})=><li key={station.id}><button type="button" className={`station-entry ${selected!==null&&members.includes(selected)?'selected':''}`} aria-pressed={selected!==null&&members.includes(selected)} onClick={()=>onSelect(station.id)}><span className="station-row-main"><span className="station-row-name">{station.name}</span><span className="station-route-chips">{routeNames.length?routeNames.map(name=><small key={name}>{name}</small>):<small className="unassigned">路線未所属</small>}</span></span><span className="station-platform-count" title={platformCount?`${platformCount}ホーム`:"ホーム情報なし"}>{platformCount||"—"}<span className="station-sr-only">ホーム</span></span></button></li>)}</ul>{!visible.length&&<p className="empty-state">一致する駅がありません</p>}</>:<p className="empty-state">まだ駅はありません。<br/>「建設」の「駅設置」から線路上に配置できます。</p>}
 </>;
}
