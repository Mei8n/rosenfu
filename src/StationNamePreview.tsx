import type {ReferencePack,ReferenceStationRepository} from './referenceStations';
import {automaticStationName} from './automaticStationName';
export function StationNamePreview({pack,repository,point,existingNames}:{pack:ReferencePack|null;repository:ReferenceStationRepository|null;point:{latitude:number;longitude:number}|null;existingNames:string[]}){
 const result=automaticStationName(point,repository,existingNames);
 return <section className="automatic-station-name" aria-label="駅名の自動設定"><p>設置時の仮駅名：<strong>{result.name}</strong></p><p className="field-hint">名前の入力は不要です。設置後に変更できます。</p>{result.candidate?<p>参考駅 {result.candidate.name} / 約{Math.round(result.candidate.distanceM)}m・直線距離。最寄りの参考名であり、同一駅や正式名称を保証しません。</p>:<p>近傍の参照駅名を取得できないため、既定名を使います。</p>}<p className="field-hint">読み込んだローカル参照駅パックの半径1km以内のみ。町名・住所などの地名検索は未対応です。ネット接続は不要です。</p>{pack&&<p className="field-hint">出典: {pack.dataset.source} / {pack.dataset.revision} / <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener">© OpenStreetMap contributors</a></p>}</section>;
}
