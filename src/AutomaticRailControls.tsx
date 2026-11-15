import type {ReferenceCopyState} from './useReferenceCopy';
import type {RailStatus} from './useAutomaticRail';
import {alignmentLength} from './alignment';
export function AutomaticRailControls({copy,status,retry,enabled,canStart,zoom,truncated}:{copy:ReferenceCopyState;status:RailStatus;retry:()=>void;enabled:boolean;canStart:boolean;zoom:number;truncated:boolean}){
 return <section aria-label="実在線路の自動追従" className="reference-controls"><h4>実在線路に沿う</h4><p>日本の参照線路網を自動取得します。点線は参考データで、所有・建設済み線路ではありません。</p>
 {status==='loading'&&<p role="status">日本の線路網を取得中…</p>}{status==='unknown'&&<p>線路coverageは未確認です。</p>}{status==='failed'&&<button disabled={!enabled} onClick={retry}>線路データを再取得</button>}
 {status==='ready'&&(zoom<14||truncated)&&<p>地図を拡大して点線を表示してください。</p>}
 <button disabled={!enabled||!canStart||status!=='ready'||copy.active||zoom<14||truncated} onClick={copy.begin}>実在線路の追従を開始</button>
 {copy.active&&<><p>{copy.picks.length?'起点選択済み。終点をクリックしてください。':'起点をクリックしてください。'} 表示された点線から12px以内を選べます。</p><button disabled={!enabled||copy.busy||!copy.picks.length} onClick={copy.undo}>参考点を戻す</button><button disabled={!enabled||copy.busy||!copy.redo.length} onClick={copy.redoPick}>参考点をやり直す</button>
 {copy.plan.build&&<><p>約{Math.round(alignmentLength(copy.plan.build.points))}m / {copy.plan.build.provenance.network_path?.length}区間。元graphの点列を保持します。</p><p>出典: TrainsWorld / noritetsu / OpenStreetMap、版 {copy.plan.build.sourceRecord.revision}。元OSM way IDは未収録。</p><label><input type="checkbox" disabled={!enabled||copy.busy} checked={copy.confirmed} onChange={e=>copy.setConfirmed(e.target.checked)}/>形状・出典を確認し、ゲーム内の敷設計画に追加します</label><button disabled={!enabled||copy.busy||!copy.confirmed||!copy.plan.build.name} onClick={()=>void copy.build()}>選択中の計画に追加</button></>}
 <button disabled={!enabled} onClick={copy.clear}>実在形状に沿う計画を破棄</button></>}
 {copy.busy&&<p role="status">参照線路の接続経路を探索中…</p>}{copy.message&&<p role="status">{copy.message}</p>}
 <details><summary>対象・出典と制約</summary><p>取得済み日本graph内の接続節点による最短経路です。交差や近接だけでは接続しません。最大256区間・4096点。網羅性、進行方向、分岐器の通過可能方向、曲線規格や高さは保証しません。橋・地下の高さも自動設定しません。OSM layerをゲームの高さへ変換しません。</p><p>元graphは供給元で簡略化済みです。参照点線と追加する形状は同じ座標列を使います。隣接国の自動取得・接続と既設線への延伸は未対応です。出典は保存後も保持します。</p><a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener noreferrer">© OpenStreetMap contributors / ODbL</a></details></section>;
}
