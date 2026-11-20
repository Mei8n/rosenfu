import {useEffect,useId,useRef} from "react";
import "./UnsavedCloseDialog.css";

/** Keep selection, map gestures and keyboard edits inert during confirmation. */
export function TrackDeleteDialog({busy,blocked,error,onCancel,onDelete}:{busy:boolean;blocked:boolean;error:string;onCancel:()=>void;onDelete:()=>void}){
 const dialog=useRef<HTMLDialogElement>(null),cancel=useRef<HTMLButtonElement>(null),title=useId();
 useEffect(()=>{const previous=document.activeElement,element=dialog.current!;element.showModal();cancel.current?.focus();return()=>{element.close();if(previous instanceof HTMLElement&&previous.isConnected)previous.focus();};},[]);
 return <dialog ref={dialog} className="unsaved-close-dialog" aria-labelledby={title} onCancel={event=>{event.preventDefault();if(!busy)onCancel();}} onKeyDown={event=>event.stopPropagation()}>
  <h2 id={title}>選択した線路全体を削除しますか？</h2>
  {error&&<p role="alert">{error}</p>}
  <div className="close-dialog-actions"><button ref={cancel} type="button" className="secondary" disabled={busy} onClick={onCancel}>キャンセル</button><button type="button" className="danger" disabled={busy||blocked} onClick={onDelete}>線路を削除</button></div>
 </dialog>;
}
