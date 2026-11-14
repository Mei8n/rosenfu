import {useEffect,useId,useRef} from "react";
import "./UnsavedCloseDialog.css";
type Props={companyName:string;currentCompanyName:string;busy:boolean;invalid:boolean;error:string;onCancel:()=>void;onConfirm:()=>void};
/** Replacement is never implicit: focus starts on cancel, background stays inert. */
export function SaveReplacementDialog({companyName,currentCompanyName,busy,invalid,error,onCancel,onConfirm}:Props){
 const dialog=useRef<HTMLDialogElement>(null),cancel=useRef<HTMLButtonElement>(null),title=useId(),description=useId();
 useEffect(()=>{const previous=document.activeElement;dialog.current!.showModal();cancel.current?.focus();return()=>{dialog.current?.close();if(previous instanceof HTMLElement&&previous.isConnected)previous.focus();};},[]);
 return <dialog ref={dialog} className="unsaved-close-dialog" aria-labelledby={title} aria-describedby={description} onCancel={event=>{event.preventDefault();if(!busy)onCancel();}} onKeyDown={event=>event.stopPropagation()}>
  <h2 id={title}>保存済みのワールドを置き換えますか？</h2>
  <p id={description}>保存済みの「{companyName}」を、いま編集中の「{currentCompanyName}」で置き換えます。線路・駅・未着工計画を含むワールド全体が対象です。</p>
  <p>直前の手動保存はバックアップに残します。以前のバックアップは更新されます。自動保存ファイルは変更しません。</p>
  {error&&<p role="alert" className="close-dialog-error">{error}</p>}
  <div className="close-dialog-actions"><button ref={cancel} className="secondary" disabled={busy} onClick={onCancel}>置き換えず編集に戻る</button><button className="danger" disabled={busy||invalid} onClick={onConfirm}>{busy?"処理中…":"バックアップして置き換える"}</button></div>
 </dialog>;
}
