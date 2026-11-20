import {useState} from "react";
import "./SaveSlots.css";
export type SaveSlot = {id:string;name:string;loadable:boolean;error?:string|null;updated_at?:number|null};
type Props={slots:SaveSlot[];current:string|null;loading:boolean;error:string;busy:boolean;canSave:boolean;onRefresh:()=>void;onLoad:(id:string)=>void;onSaveAs:()=>void;onNew:()=>void;deletedSlots?:SaveSlot[];onDelete?:(id:string)=>Promise<void>;onRestore?:(id:string)=>Promise<void>;onRename?:(id:string,expectedName:string,name:string)=>Promise<void>;onSave?:()=>void;saveDisabled?:boolean};
function savedTime(value:number|null|undefined){
 if(value==null||!Number.isFinite(value))return null;
 const date=new Date(value*1000);
 return Number.isNaN(date.getTime())?null:date;
}
export function SaveSlots({slots,current,loading,error,busy,canSave,onRefresh,onLoad,onSaveAs,onNew,onSave,saveDisabled,deletedSlots=[],onDelete,onRestore,onRename}:Props){
 const [renaming,setRenaming]=useState<SaveSlot|null>(null);const [name,setName]=useState("");
 const [pending,setPending]=useState<SaveSlot|null>(null);const [failure,setFailure]=useState("");const [working,setWorking]=useState(false);
 async function rename(){if(working||busy||!renaming||!onRename)return;setWorking(true);setFailure("");try{await onRename(renaming.id,renaming.name,name.trim());setRenaming(null);}catch(error){setFailure(error instanceof Error?error.message:String(error));}finally{setWorking(false);}}
 async function change(id:string,restore:boolean){if(working||busy)return;setWorking(true);setFailure("");try{await (restore?onRestore?.(id):onDelete?.(id));setPending(null);}catch(error){setFailure(error instanceof Error?error.message:String(error));}finally{setWorking(false);}}
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
    <div className="save-slot-card-actions"><button type="button" aria-label={slot.name+(active?"（現在）":"")} aria-current={active?"true":undefined} disabled={busy||loading||!slot.loadable} onClick={()=>onLoad(slot.id)}>読み込む</button>{onRename&&slot.id!=="legacy"&&<button type="button" className="secondary" disabled={busy||loading||working} aria-label={`${slot.name}の名前を変更`} onClick={()=>{setFailure("");setName(slot.name);setRenaming(slot);}}>名前を変更</button>}{onDelete&&<button type="button" className="secondary" disabled={busy||loading||working} aria-label={`${slot.name}を削除`} onClick={()=>{setFailure("");setPending(slot);}}>削除</button>}</div>
   </li>;
  })}</ul>
  {deletedSlots.length>0&&<details className="deleted-saves"><summary>削除済みデータ ({deletedSlots.length})</summary><ul>{deletedSlots.map(slot=><li key={slot.id}><span>{slot.name}</span><button type="button" disabled={busy||working} onClick={()=>void change(slot.id,true)}>復元</button></li>)}</ul></details>}
  {renaming&&<div className="company-setup-backdrop"><form className="company-setup" role="dialog" aria-modal="true" aria-label="セーブデータの名前変更" onSubmit={event=>{event.preventDefault();void rename();}}><h3>セーブデータの名前変更</h3><input aria-label="セーブデータ名" autoComplete="off" value={name} disabled={working||busy} onChange={event=>setName(event.target.value)}/><button type="button" disabled={working||busy} onClick={()=>{setRenaming(null);setFailure("");}}>キャンセル</button><button type="submit" disabled={working||busy||!name.trim()||Array.from(name.trim()).length>100||/[\x00-\x1f\x7f-\x9f]/.test(name)}>変更する</button>{failure&&<p role="alert">{failure}</p>}</form></div>}
  {pending&&<div className="company-setup-backdrop"><section className="company-setup" role="dialog" aria-modal="true" aria-label="セーブデータの削除"><h3>「{pending.name}」を削除しますか？</h3><p>保存一覧から取り除きます。削除済みデータから復元できます。{pending.id==="legacy"&&"旧形式は手動保存だけを移動し、自動保存とバックアップは残します。"}{pending.id===current&&"現在の編集中の内容は画面に残り、次回保存は別のデータになります。"}</p><button disabled={working||busy} onClick={()=>setPending(null)}>キャンセル</button><button disabled={working||busy} onClick={()=>void change(pending.id,false)}>削除する</button>{failure&&<p role="alert">{failure}</p>}</section></div>}
  {!pending&&!renaming&&failure&&<p role="alert">{failure}</p>}
  {!loading&&!error&&slots.length===0&&<p>保存済みワールドはありません</p>}
 </section>;
}
