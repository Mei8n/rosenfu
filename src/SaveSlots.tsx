import "./SaveSlots.css";
export type SaveSlot = {id:string;name:string;loadable:boolean;error?:string|null;updated_at?:number|null};
type Props={slots:SaveSlot[];current:string|null;loading:boolean;error:string;busy:boolean;canSave:boolean;onRefresh:()=>void;onLoad:(id:string)=>void;onSaveAs:()=>void;onNew:()=>void;onSave?:()=>void;saveDisabled?:boolean};
function savedTime(value:number|null|undefined){
 if(value==null||!Number.isFinite(value))return null;
 const date=new Date(value*1000);
 return Number.isNaN(date.getTime())?null:date;
}
export function SaveSlots({slots,current,loading,error,busy,canSave,onRefresh,onLoad,onSaveAs,onNew,onSave,saveDisabled}:Props){
 return <section className="save-slots" aria-label="保存済みワールド">
  <div className="save-slot-actions">
   {onSave&&<button type="button" disabled={busy||saveDisabled} onClick={onSave}>ワールドを保存</button>}
   <button type="button" disabled={busy||!canSave} onClick={onSaveAs}>別のセーブとして保存</button>
   <button type="button" disabled={busy} onClick={onNew}>新しいワールド</button>
   <button type="button" className="secondary" disabled={busy||loading} onClick={onRefresh}>一覧を更新</button>
  </div>
  {loading&&<p role="status">保存一覧を読み込み中</p>}{error&&<p role="alert">{error}</p>}
  <ul className="save-slot-list">{slots.map(slot=>{
   const active=current===slot.id,time=savedTime(slot.updated_at);
   return <li className={`save-slot-card${active?" is-current":""}`} key={slot.id}>
    <div className="save-slot-heading"><strong>{slot.name}</strong>{active&&<span className="save-slot-current">現在のワールド</span>}</div>
    {time&&<time dateTime={time.toISOString()}>更新日時 {time.toLocaleString("ja-JP")}</time>}
    {!slot.loadable&&<small>{slot.error||"この保存は読み込めません"}</small>}
    <div className="save-slot-card-actions"><button type="button" aria-label={slot.name+(active?"（現在）":"")} aria-current={active?"true":undefined} disabled={busy||loading||!slot.loadable} onClick={()=>onLoad(slot.id)}>読み込む</button></div>
   </li>;
  })}</ul>
  {!loading&&!error&&slots.length===0&&<p>保存済みワールドはありません</p>}
 </section>;
}
