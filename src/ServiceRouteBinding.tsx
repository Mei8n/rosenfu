import {useEffect,useRef,useState} from 'react';
export type RouteBinding={kind:'independent'}|{kind:'route';route_id:string}|{kind:'routes';route_ids:string[]};
export type ServiceRouteBindingRecord={service_id:string;binding:RouteBinding};
const bindingKey=(binding?:RouteBinding)=>binding?.kind==='route'?`route:${binding.route_id}`:binding?.kind==='routes'?'routes:'+JSON.stringify([...binding.route_ids].sort()):binding?.kind??'';
export const sameRouteBinding=(a?:RouteBinding,b?:RouteBinding)=>bindingKey(a)===bindingKey(b);
export function ServiceRouteBindingEditor({binding,routes,enabled,onDirty,onSave}:{binding?:RouteBinding;routes:{id:string;name:string}[];enabled:boolean;onDirty:(dirty:boolean)=>void;onSave:(binding:RouteBinding)=>Promise<void>}){
 const current=bindingKey(binding),[choice,setChoice]=useState(binding?.kind==='routes'?'routes':current),[routeIds,setRouteIds]=useState(binding?.kind==='routes'?binding.route_ids:[]),[error,setError]=useState(''),[saving,setSaving]=useState(false),pending=useRef(false);
 const draft:RouteBinding|undefined=choice==='independent'?{kind:'independent'}:choice==='routes'?{kind:'routes',route_ids:routeIds}:choice.startsWith('route:')?{kind:'route',route_id:choice.slice(6)}:undefined;
 const changed=bindingKey(draft)!==current,valid=!!draft&&(draft.kind!=='routes'||(routeIds.length>=2&&routeIds.length<=64));
 useEffect(()=>{setChoice(binding?.kind==='routes'?'routes':current);setRouteIds(binding?.kind==='routes'?binding.route_ids:[]);setError('');},[current]);
 useEffect(()=>{onDirty(changed);},[changed,onDirty]);
 function reset(){setChoice(binding?.kind==='routes'?'routes':current);setRouteIds(binding?.kind==='routes'?binding.route_ids:[]);setError('');}
 async function save(){if(!enabled||pending.current||!valid||!changed||!draft)return;pending.current=true;setSaving(true);setError('');try{await onSave(draft);}catch(e){setError(String(e));}finally{pending.current=false;setSaving(false);}}
 return <section aria-label="系統の路線関連付け"><label>関連する路線<select value={choice} disabled={!enabled||saving} onChange={e=>{setChoice(e.target.value);setError('');}}><option value="" disabled>未確認</option><option value="independent">路線から独立</option><option value="routes">複数路線へ直通</option>{routes.map(route=><option key={route.id} value={`route:${route.id}`}>{route.name}</option>)}</select></label>
 {choice==='routes'&&<fieldset disabled={!enabled||saving} aria-label="直通する関連路線">{routes.map(route=><label key={route.id}><input type="checkbox" checked={routeIds.includes(route.id)} onChange={event=>setRouteIds(ids=>event.target.checked?[...ids,route.id]:ids.filter(id=>id!==route.id))}/>{route.name}</label>)}</fieldset>}
 {!binding&&<p role="status">路線との関連は未確認です。自動では割り当てません。</p>}
 <button type="button" disabled={!enabled||saving||!valid||!changed} onClick={()=>void save()}>関連付けを適用</button><button type="button" className="secondary" disabled={saving||!changed} onClick={reset}>関連付けの変更を破棄</button>{error&&<p role="alert">{error}</p>}</section>;
}
