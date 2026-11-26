import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import type {GeoPoint} from "./alignment";

type SeamOption={catalogKey:string;routeId:string;lengthM:number;points:GeoPoint[]};
type MissingTile={revision:string;z:14;x:number;y:number;raw_sha256:string;decoder_rule:string};
type Acquisition={catalogKey:string;missing:MissingTile[]};
type Available={expectedWorldToken:string;options:SeamOption[];acquisitions?:Acquisition[]};
export type AcquireSeamBusArgs=Acquisition&{expectedWorldToken:string};
export type InstallSeamBusArgs={expectedWorldToken:string;catalogKey:string;routeId:string;name:string};

export function InstalledSeamBusPanel({scope,enabled,commit,acquire}:{
  scope:unknown;enabled:boolean;commit:(args:InstallSeamBusArgs)=>Promise<unknown>;acquire?:(args:AcquireSeamBusArgs)=>Promise<unknown>;
}){
  const [available,setAvailable]=useState<{scope:unknown;data:Available}>();
  const [selected,setSelected]=useState("");
  const [name,setName]=useState("境界横断バス");
  const [error,setError]=useState("");
  const [pending,setPending]=useState(false);
  useEffect(()=>{
    let current=true;
    setAvailable(undefined);setSelected("");setError("");
    void invoke<Available>("installed_seam_bus_options").then(data=>{
      if(current&&(data?.options?.length||data?.acquisitions?.length)){
        setAvailable({scope,data});setSelected(data.options[0]?.catalogKey??"");
      }
    },()=>{});
    return ()=>{current=false;};
  },[scope]);
  const data=available?.scope===scope?available?.data:undefined;
  const option=data?.options.find(item=>item.catalogKey===selected);
  const acquisition=acquire?data?.acquisitions?.[0]:undefined;
  if(!option&&!acquisition)return null;
  const label=(item:SeamOption)=>{
    const start=item.points[0];
    return start?`${start.latitude.toFixed(4)}, ${start.longitude.toFixed(4)} · 片道 ${Math.round(item.lengthM)} m`:`片道 ${Math.round(item.lengthM)} m`;
  };
  async function install(){
    if(!enabled||pending||!option||!data||!name.trim())return;
    setPending(true);setError("");
    try{await commit({expectedWorldToken:data.expectedWorldToken,catalogKey:option.catalogKey,routeId:option.routeId,name:name.trim()});}
    catch(e){setError(e instanceof Error?e.message:typeof e==="string"?e:"確認済みの候補を作成できませんでした。");}
    finally{setPending(false);}
  }
  async function fetchApprovedRoad(){
    if(!enabled||pending||!acquisition||!data||!acquire)return;
    setPending(true);setError("");
    try{await acquire({...acquisition,expectedWorldToken:data.expectedWorldToken});}
    catch(e){setError(e instanceof Error?e.message:typeof e==="string"?e:"確認済みの道路を取得できませんでした。");}
    finally{setPending(false);}
  }
  if(!option||!data)return <section aria-label="確認済みの地図境界横断バス">
    <h4>確認済みの地図境界横断バス</h4>
    <p>この区間に必要な道路データを取得します。元の版と一致した場合だけワールドに保存します。</p>
    <button type="button" disabled={!enabled||pending} onClick={()=>void fetchApprovedRoad()}>確認済み区間の道路を取得</button>
    {error&&<p role="alert">{error}</p>}
  </section>;
  return <section aria-label="確認済みの地図境界横断バス">
    <h4>確認済みの地図境界横断バス</h4>
    <p>ワールドに保存済みの道路から、確認できた位置にだけ片道で作成します。</p>
    {data.options.length>1?<label>確認済み区間<select disabled={!enabled||pending} value={option.catalogKey} onChange={event=>setSelected(event.target.value)}>
      {data.options.map(item=><option key={item.catalogKey} value={item.catalogKey}>{label(item)}</option>)}
    </select></label>:<p>{label(option)}</p>}
    <label>境界横断バス系統名<input value={name} maxLength={64} disabled={!enabled||pending} onChange={event=>setName(event.target.value)}/></label>
    <button type="button" disabled={!enabled||pending||!name.trim()} onClick={()=>void install()}>確認済み区間に作成</button>
    {error&&<p role="alert">{error}</p>}
  </section>;
}
