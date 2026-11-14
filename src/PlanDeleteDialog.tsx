import {useEffect,useId,useRef} from "react";
import "./UnsavedCloseDialog.css";

export function PlanDeleteDialog({name,onCancel,onDelete}:{name:string;onCancel:()=>void;onDelete:()=>void}){
 const dialog=useRef<HTMLDialogElement>(null),cancel=useRef<HTMLButtonElement>(null),title=useId();
 useEffect(()=>{const previous=document.activeElement,element=dialog.current!;element.showModal();cancel.current?.focus();return()=>{element.close();if(previous instanceof HTMLElement&&previous.isConnected)previous.focus();};},[]);
 return <dialog ref={dialog} className="unsaved-close-dialog" aria-label="計画を削除" aria-describedby={title} onCancel={event=>{event.preventDefault();onCancel();}} onKeyDown={event=>event.stopPropagation()}>
  <h2 id={title}>{name}を削除しますか？</h2>
  <p>この計画の未着工の線路と駅を削除します。建設済みの線路や駅は残ります。</p>
  <div className="close-dialog-actions"><button ref={cancel} type="button" className="secondary" onClick={onCancel}>戻る</button><button type="button" className="danger" onClick={onDelete}>削除する</button></div>
 </dialog>;
}
