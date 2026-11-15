import {useEffect,useRef,useState} from 'react';
export type RouteBinding={kind:'independent'}|{kind:'route';route_id:string};
export type ServiceRouteBindingRecord={service_id:string;binding:RouteBinding};
const bindingKey=(binding?:RouteBinding)=>binding?.kind==='route'?`route:${binding.route_id}`:binding?.kind??'';
export function ServiceRouteBindingEditor({binding,routes,enabled,onDirty,onSave}:{binding?:RouteBinding;routes:{id:string;name:string}[];enabled:boolean;onDirty:(dirty:boolean)=>void;onSave:(binding:RouteBinding)=>Promise<void>}){
 const current=bindingKey(binding),[choice,setChoice]=useState(current),[error,setError]=useState(''),[saving,setSaving]=useState(false),pending=useRef(false);
 useEffect(()=>{setChoice(current);setError('');},[current]);
 useEffect(()=>{onDirty(choice!==current);},[choice,current,onDirty]);
 async function save(){if(!enabled||pending.current||!choice||choice===current)return;pending.current=true;setSaving(true);setError('');try{await onSave(choice==='independent'?{kind:'independent'}:{kind:'route',route_id:choice.slice(6)});}catch(e){setError(String(e));}finally{pending.current=false;setSaving(false);}}
 return <section aria-label="系統の路線関連付け"><label>関連する路線<select value={choice} disabled={!enabled||saving} onChange={e=>{setChoice(e.target.value);setError('');}}><option value="" disabled>未確認</option><option value="independent">路線から独立</option>{routes.map(route=><option key={route.id} value={`route:${route.id}`}>{route.name}</option>)}</select></label>
 {!binding&&<p role="status">路線との関連は未確認です。自動では割り当てません。</p>}
 <button type="button" disabled={!enabled||saving||!choice||choice===current} onClick={()=>void save()}>関連付けを適用</button><button type="button" className="secondary" disabled={saving||choice===current} onClick={()=>{setChoice(current);setError('');}}>関連付けの変更を破棄</button>{error&&<p role="alert">{error}</p>}</section>;
}
