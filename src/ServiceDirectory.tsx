import {useMemo,useState} from 'react';
import {firstByKey} from './mapLookupIndex';
import {ServiceBadge,initialServiceSymbol,type ServiceSymbolShape} from './ServiceAppearanceControls';
import './directoryPanel.css';
type Service={id:string;name:string};
type Appearance={service_id:string;color:string;shape?:ServiceSymbolShape;symbol?:string|null};
const NO_APPEARANCES:Appearance[]=[];
export function ServiceDirectory({services,appearances=NO_APPEARANCES,selected,enabled,onChoose,onCreate}:{services:Service[];appearances?:Appearance[];selected:string|null;enabled:boolean;onChoose:(id:string|null)=>void;onCreate:()=>void}){
 const [query,setQuery]=useState('');
 const normalized=query.trim().normalize('NFKC').toLocaleLowerCase('ja');
 const visible=useMemo(()=>normalized?services.filter(service=>service.name.normalize('NFKC').toLocaleLowerCase('ja').includes(normalized)):services,[services,normalized]);
 const appearancesByService=useMemo(()=>firstByKey(appearances,row=>row.service_id),[appearances]);
 return <div className="service-directory network-directory">
  <div className="directory-toolbar"><span className="directory-total">{services.length}系統</span><button type="button" className="secondary" disabled={!enabled} onClick={onCreate}>新規系統</button></div>
  <label className="directory-search">系統を探す<input type="search" placeholder="系統名" value={query} onChange={event=>setQuery(event.target.value)}/></label>
  <table aria-label="系統一覧"><thead><tr><th>名前</th><th>乗客</th><th>収支</th><th>車両数</th></tr></thead><tbody>{visible.map(service=>{const appearance=appearancesByService.get(service.id);return <tr key={service.id}><th><button type="button" className={`route-choice ${service.id===selected?'selected':''}`} disabled={!enabled} onClick={()=>onChoose(service.id===selected?null:service.id)} aria-label={`系統編集: ${service.name}`} aria-pressed={service.id===selected}><ServiceBadge color={appearance?.color??'#3b82f6'} shape={appearance?.shape??'circle'} symbol={appearance?.symbol??initialServiceSymbol(service.name)}/><span>{service.name}</span></button></th>{['乗客','収支','車両数'].map(label=><td key={label} aria-label={`${label}: 未実装`}>—</td>)}</tr>;})}</tbody></table>
  {!visible.length&&<p className="empty-state">{services.length?'一致する系統がありません':'まだ系統はありません。新規系統から地図上の駅を選択してください。'}</p>}
  {visible.length>0&&<small className="directory-note">乗客・収支・車両数は今後対応予定です</small>}
 </div>;
}
