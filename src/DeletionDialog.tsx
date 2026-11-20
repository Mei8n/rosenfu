import {useEffect,useId,useRef} from 'react';
import './UnsavedCloseDialog.css';
export function DeletionDialog({label,busy,blocked,error,defaultDelete=false,onCancel,onDelete}:{label:string;busy:boolean;blocked:string;error:string;defaultDelete?:boolean;onCancel:()=>void;onDelete:()=>void}){
 const dialog=useRef<HTMLDialogElement>(null),cancel=useRef<HTMLButtonElement>(null),remove=useRef<HTMLButtonElement>(null),title=useId();
 useEffect(()=>{const previous=document.activeElement;dialog.current!.showModal();(defaultDelete&&!busy&&!blocked?remove:cancel).current?.focus();return()=>{dialog.current?.close();if(previous instanceof HTMLElement&&previous.isConnected)previous.focus();};},[]);
 return <dialog ref={dialog} className="unsaved-close-dialog" aria-labelledby={title} onCancel={event=>{event.preventDefault();if(!busy)onCancel();}} onKeyDown={event=>{
  event.stopPropagation();
  // Keep Enter local to the focused action; ignore held keys and IME confirmation.
  if(defaultDelete&&event.key==='Enter'&&event.target===remove.current){
   event.preventDefault();
   if(!busy&&!blocked&&!event.repeat&&!event.nativeEvent.isComposing&&event.keyCode!==229)onDelete();
  }
 }}><h2 id={title}>{label}を削除しますか？</h2>{(blocked||error)&&<p role="alert">{blocked||error}</p>}<div className="close-dialog-actions"><button ref={cancel} className="secondary" disabled={busy} onClick={onCancel}>キャンセル</button><button ref={remove} className="danger" disabled={busy||!!blocked} onClick={onDelete}>削除する</button></div></dialog>;
}
