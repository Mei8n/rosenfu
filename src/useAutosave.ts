import { useEffect, useRef, useState, type MutableRefObject } from "react";
import { invoke } from "@tauri-apps/api/core";
const key="rosenfu.autosave-minutes";
type Permission="checking"|"failed"|"blocked"|"allowed";
export function useAutosave({ready,dirty,world,epoch,restoredEpoch=-1,busyRef,setBusy}:{ready:boolean;dirty:boolean;world:unknown;epoch:number;restoredEpoch?:number;busyRef:MutableRefObject<boolean>;setBusy:(busy:boolean)=>void}) {
 const [minutes,setMinutesState]=useState(()=>{try{const n=Number(localStorage.getItem(key)??5);return [0,1,5,10].includes(n)?n:5;}catch{return 5;}});
 const [message,setMessage]=useState("自動保存データを確認中…");const [available,setAvailable]=useState(false);
 const [permission,setPermission]=useState<Permission>("checking");const [retry,setRetry]=useState(0);
 const permissionRef=useRef<{epoch:number;mode:Permission}>({epoch:-1,mode:"checking"});
 const latest=useRef({ready,dirty,world,epoch});latest.current={ready,dirty,world,epoch};
 const saved=useRef<unknown>(null);const mounted=useRef(true);
 const restoredGrant=useRef<{epoch:number;retry:number}|null>(null);
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
 function setMinutes(value:number){if(![0,1,5,10].includes(value))return;setMinutesState(value);try{localStorage.setItem(key,String(value));}catch{setMessage("自動保存間隔を端末に保存できませんでした。この起動中のみ適用します。");}}
 function updatePermission(mode:Permission,forEpoch=epoch){permissionRef.current={epoch:forEpoch,mode};setPermission(mode);}
 useEffect(()=>{
  saved.current=null;updatePermission("checking");setAvailable(false);
  if(!ready)return;
  if(restoredEpoch===epoch){
   if(restoredGrant.current?.epoch!==epoch)restoredGrant.current={epoch,retry};
   // A restored baseline authorizes the initial cycle, not an explicit recheck
   // after a save conflict. Keep StrictMode's repeated initial effect harmless.
   if(restoredGrant.current.retry===retry){updatePermission("allowed");setAvailable(true);setMessage("復元したワールドの自動保存を継続します。");return;}
  }
  let active=true;setMessage("自動保存データを確認中…");
  void invoke<{available:boolean}>("autosave_status").then(result=>{
   if(!active)return;if(typeof result?.available!=="boolean")throw Error("確認結果が不正です。");
   setAvailable(result.available);updatePermission(result.available?"blocked":"allowed");
   setMessage(result.available?"以前の自動保存を保護するため、自動保存を保留しています。復元するか、置き換えを許可してください。":"自動保存待機中");
  }).catch(()=>{if(active){updatePermission("failed");setMessage("自動保存データの確認に失敗しました。確認できるまで書き込みを停止します。");}});
  return()=>{active=false;};
 },[ready,epoch,restoredEpoch,retry]);
 async function allowReplacement(){
  if(permissionRef.current.epoch!==epoch||permissionRef.current.mode!=="blocked"||busyRef.current)return;
  const requestedEpoch=epoch;busyRef.current=true;setBusy(true);
  try{
   await invoke("allow_autosave_replacement");
   if(!mounted.current||latest.current.epoch!==requestedEpoch)return;
   updatePermission("allowed");setMessage("このワールドで自動保存を開始します。次回の自動保存で以前のデータを置き換えます。");
  }catch(error){if(mounted.current&&latest.current.epoch===requestedEpoch){updatePermission("failed");setMessage("自動保存の置き換え確認に失敗しました: "+(error instanceof Error?error.message:String(error)));}}
  finally{busyRef.current=false;if(mounted.current)setBusy(false);}
 }
 function retryStatus(){if(busyRef.current)return;updatePermission("checking");setRetry(value=>value+1);}
 useEffect(()=>{
  if(!ready||minutes===0)return;
  const timer=setInterval(()=>{
   const current=latest.current;
   if(!current.ready||!current.dirty||!current.world||current.world===saved.current||busyRef.current||permissionRef.current.epoch!==current.epoch||permissionRef.current.mode!=="allowed")return;
   busyRef.current=true;setBusy(true);setMessage("自動保存中…");
   void invoke("autosave_world").then(()=>{if(!mounted.current||latest.current.epoch!==current.epoch)return;saved.current=current.world;setAvailable(true);setMessage("自動保存しました（"+new Date().toLocaleTimeString()+"）。手動保存とは別のデータです。");}).catch(error=>{if(mounted.current&&latest.current.epoch===current.epoch){updatePermission("failed",current.epoch);setMessage("自動保存に失敗しました。データを再確認してから再開してください: "+(error instanceof Error?error.message:String(error)));}}).finally(()=>{busyRef.current=false;if(mounted.current)setBusy(false);});
  },minutes*60000);
  return()=>clearInterval(timer);
 },[ready,minutes,busyRef,setBusy]);
 return {minutes,setMinutes,message,available,permission,allowReplacement,retryStatus};
}
