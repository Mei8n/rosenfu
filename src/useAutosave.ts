import { useEffect, useRef, useState, type MutableRefObject } from "react";
import type {Camera} from "./map";
import type {ConstructionPlans} from "./constructionPlans";
import { invoke } from "@tauri-apps/api/core";
const AUTOSAVE_INTERVAL_MS=120000;
type Permission="checking"|"failed"|"allowed";
export function useAutosave({slotId,ready,dirty,continuous=false,world,constructionPlans,viewState,epoch,restoredEpoch=-1,busyRef,setBusy}:{slotId:string|null;ready:boolean;dirty:boolean;continuous?:boolean;world:unknown;constructionPlans?:ConstructionPlans;viewState?:Camera;epoch:number;restoredEpoch?:number;busyRef:MutableRefObject<boolean>;setBusy:(busy:boolean)=>void}) {
 const [message,setMessage]=useState("自動保存データを確認中…");const [available,setAvailable]=useState(false);
 const [permission,setPermission]=useState<Permission>("checking");const [retry,setRetry]=useState(0);
 const permissionRef=useRef<{epoch:number;mode:Permission}>({epoch:-1,mode:"checking"});
 const latest=useRef({slotId,ready,dirty,continuous,world,constructionPlans,viewState,epoch});latest.current={slotId,ready,dirty,continuous,world,constructionPlans,viewState,epoch};
 const saved=useRef<unknown>(null);const mounted=useRef(true);
 const restoredGrant=useRef<{epoch:number;retry:number}|null>(null);
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
 function updatePermission(mode:Permission,forEpoch=epoch){permissionRef.current={epoch:forEpoch,mode};setPermission(mode);}
 useEffect(()=>{
  saved.current=null;updatePermission("checking");setAvailable(false);
  if(!ready||!slotId)return;
  if(restoredEpoch===epoch){
   if(restoredGrant.current?.epoch!==epoch)restoredGrant.current={epoch,retry};
   // A restored baseline authorizes the initial cycle, not an explicit recheck
   // after a save conflict. Keep StrictMode's repeated initial effect harmless.
   if(restoredGrant.current.retry===retry){updatePermission("allowed");setAvailable(true);setMessage("復元したワールドの自動保存を継続します。");return;}
  }
  let active=true;setMessage("自動保存データを確認中…");
  void invoke<{available:boolean}>("autosave_status",{slotId}).then(result=>{
   if(!active)return;if(typeof result?.available!=="boolean")throw Error("確認結果が不正です。");
   setAvailable(result.available);updatePermission("allowed");
   setMessage("自動保存は有効です（2分ごと）");
  }).catch(()=>{if(active){updatePermission("failed");setMessage("自動保存データの確認に失敗しました。確認できるまで書き込みを停止します。");}});
  return()=>{active=false;};
 },[ready,slotId,epoch,restoredEpoch,retry]);
 function retryStatus(){if(busyRef.current)return;updatePermission("checking");setRetry(value=>value+1);}
 useEffect(()=>{
  if(!ready||!slotId)return;
  const timer=setInterval(()=>{
   const current=latest.current;
   if(!current.slotId||!current.ready||!current.dirty||!current.world||(!current.continuous&&current.world===saved.current)||busyRef.current||permissionRef.current.epoch!==current.epoch||permissionRef.current.mode!=="allowed")return;
   busyRef.current=true;setBusy(true);setMessage("自動保存中…");
   void (current.constructionPlans||current.viewState?invoke("autosave_world",{slotId:current.slotId,...(current.constructionPlans?{constructionPlans:current.constructionPlans}:{}),...(current.viewState?{viewState:current.viewState}:{})}):invoke("autosave_world",{slotId:current.slotId})).then(()=>{if(!mounted.current||latest.current.slotId!==current.slotId||latest.current.epoch!==current.epoch)return;saved.current=current.world;setAvailable(true);setMessage("自動保存しました（"+new Date().toLocaleTimeString()+"）。手動保存とは別のデータです。");}).catch(error=>{if(mounted.current&&latest.current.slotId===current.slotId&&latest.current.epoch===current.epoch){updatePermission("failed",current.epoch);setMessage("自動保存に失敗しました。データを再確認してから再開してください: "+(error instanceof Error?error.message:String(error)));}}).finally(()=>{busyRef.current=false;if(mounted.current)setBusy(false);});
  },AUTOSAVE_INTERVAL_MS);
  return()=>clearInterval(timer);
 },[ready,slotId,busyRef,setBusy]);
 return {message,available,permission,retryStatus};
}
