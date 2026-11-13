import { useMemo } from "react";
import type {ReferencePack,ReferenceStationRepository} from "./referenceStations";
export function ReferenceNameSuggestions({pack,repository,point,disabled,onChoose}:{pack:ReferencePack|null;repository:ReferenceStationRepository|null;point:{latitude:number;longitude:number}|null;disabled:boolean;onChoose:(name:string)=>void}){
 const result=useMemo(()=>point&&repository?repository.suggestNames(point):null,[repository,point?.latitude,point?.longitude]);
 return <section className="reference-name-suggestions" aria-label="新駅名の参考候補"><h4>新駅名の参考候補</h4><p className="field-hint">ローカル参考駅名のみ。周辺地名全般は未対応です。半径1km・近い順5件は暫定UI上限。網羅性・同一駅・正式名称を保証しません。</p>
 {!pack?<p>参照駅パック未導入のため候補なし。地図設定からJSONを選択できます。</p>:!point?<p>新駅のホーム位置と寸法を選ぶと参考候補を表示します。</p>:result?.status==="outside"?<p>配置位置がパック範囲外のため候補なし。</p>:result?.status==="none"?<p>利用できる近傍の参考候補なし。未収録・名称不明等の可能性があり、周辺に駅がないという意味ではありません。</p>:<ol>{result?.candidates.map(candidate=><li key={candidate.station.osmType+"/"+candidate.station.osmId}><button type="button" className="secondary" disabled={disabled} onClick={()=>onChoose(candidate.name)}>{candidate.name}を新駅名入力へ使う</button><small>約{Math.round(candidate.distanceM)}m・直線距離 / 参考駅 {candidate.station.osmType}/{candidate.station.osmId}</small></li>)}</ol>}
 {pack&&<p className="field-hint">候補の出典: {pack.dataset.source} / {pack.dataset.revision}。候補を選ぶと新駅の入力欄だけに反映します。駅設置の確定や既存駅の改名はしません。<br/><a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener">参考候補: © OpenStreetMap contributors</a></p>}</section>;
}
