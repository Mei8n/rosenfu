import { referenceStationStorage } from "./referenceStationStorage";
import type { ReferenceStationStorage } from "./referenceStationStorage";
import { useEffect,useRef,useState } from "react";
import { parseReferencePack,REFERENCE_MAX_BYTES } from "./referenceStations";
import type { ReferencePack } from "./referenceStations";
export function ReferenceStationControls({pack,visible,onPack,onVisible,total,shown,storage=referenceStationStorage}:{storage?:ReferenceStationStorage;pack:ReferencePack|null;visible:boolean;onPack:(pack:ReferencePack|null)=>void;onVisible:(visible:boolean)=>void;total:number;shown:number}){
 const [error,setError]=useState("");const [loading,setLoading]=useState(false);const generation=useRef(0);const persistenceGeneration=useRef(0);
 const [storageStatus,setStorageStatus]=useState("端末に保存した参照パックを確認中…");
 useEffect(()=>{const token=++generation.current;const persistenceToken=++persistenceGeneration.current;void storage.load().then(json=>{
  if(token!==generation.current){if(persistenceToken===persistenceGeneration.current)setStorageStatus("端末保存の確認を完了しました。現在の選択を優先しています。");return;}
  if(json!==null){const restored=parseReferencePack(json);onPack(restored);onVisible(true);setStorageStatus("端末保存から復元しました。");}else setStorageStatus("選択したパックを端末内に保存します。");
 }).catch(cause=>{if(persistenceToken===persistenceGeneration.current)setStorageStatus("端末保存の復元に失敗（現在のパックは維持）: "+String(cause));});
 return ()=>{generation.current++;persistenceGeneration.current++;};},[storage]);
 return <section className="reference-controls" aria-label="ローカル参照駅"><h4>参考駅（ローカル）</h4><p>JSONを選択すると検証後に端末内へ保存し、次回起動で復元します。ネット送信なし。ゲーム駅・背景地図・ワールド保存とは別です。</p>
 <label>参照駅JSON<input type="file" accept=".json,application/json" onChange={async event=>{
 const file=event.target.files?.[0];event.target.value="";if(!file)return;const token=++generation.current;setError("");setLoading(true);
 try {if(file.size>REFERENCE_MAX_BYTES)throw Error("5MiB以下のファイルを選択してください。");const parsed=parseReferencePack(await file.text());if(token===generation.current){onPack(parsed);onVisible(true);const persistenceToken=++persistenceGeneration.current;setStorageStatus("端末に保存中…");try{await storage.save(JSON.stringify(parsed));if(persistenceToken===persistenceGeneration.current)setStorageStatus("端末に保存済み。次回起動で復元します。");}catch(cause){if(persistenceToken===persistenceGeneration.current)setStorageStatus("端末保存に失敗。このセッションでは利用できます。前の保存は保持: "+String(cause));}}}
 catch(cause){if(token===generation.current)setError(cause instanceof Error?cause.message:String(cause));}finally{if(token===generation.current)setLoading(false);}
 }}/></label><p role="status">{storageStatus}</p>{loading&&<p role="status">参照駅を読み込み中…</p>}{error&&<p role="alert">読込失敗（現在のパックは維持）: {error}</p>}
 {pack&&<><label><input type="checkbox" checked={visible} onChange={event=>onVisible(event.target.checked)}/>参考駅を表示</label><p>{pack.dataset.id} / {pack.dataset.revision} / {pack.stations.length}件</p><p>表示範囲 {total}件・描画 {visible?shown:0}件（最大300件）。多い場合は拡大してください。</p><p>出典: {pack.dataset.source}<br/>ライセンス: {pack.dataset.license}<br/><a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener">© OpenStreetMap contributors</a></p></>}<button type="button" className="secondary" onClick={()=>{generation.current++;const persistenceToken=++persistenceGeneration.current;setLoading(false);setError("");onPack(null);setStorageStatus("端末の参照パックを削除中…");void storage.remove().then(()=>{if(persistenceToken===persistenceGeneration.current)setStorageStatus("端末の参照パックを削除しました。");}).catch(cause=>{if(persistenceToken===persistenceGeneration.current)setStorageStatus("端末データの削除に失敗。再起動すると戻る可能性があります: "+String(cause));});}}>参照パックを外す</button>
 <p>背景画像・路線スナップ・オフライン実地図は含みません。ワールド保存にも含まれません。</p></section>;
}
