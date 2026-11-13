import {useEffect,useRef} from 'react';
export const ERROR_NOTICE_MS=5000;
export function ErrorNotice({message,revision,onDismiss}:{message:string;revision:number;onDismiss:()=>void}){
 const dismiss=useRef(onDismiss);dismiss.current=onDismiss;
 useEffect(()=>{const timer=setTimeout(()=>dismiss.current(),ERROR_NOTICE_MS);return()=>clearTimeout(timer);},[message,revision]);
 return <div className="banner error" role="alert"><span>処理できませんでした: {message}</span><button type="button" className="secondary" aria-label="エラー通知を閉じる" onClick={onDismiss}>×</button></div>;
}
