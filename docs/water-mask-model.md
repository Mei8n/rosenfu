# 水域マスクの取得とモデル分類

`src/waterMaskModel.ts` は、水域専用 byte range の取得、細密行ランの復号、TWW1 世界マスクの復号、二層モデルの地点分類を行う。現時点でApp/session/IPCへ未接続であり、着工を許可・拒否する機能ではない。全国配信データの登録もしていない。

## 原典との比較と利用条件

参考にした公開原典は TrainsWorld commit `6b8ef91729e1e5812f3dcbaedd6c0a49fed6a84b`。コードを転用せず、公開データ形式と取得・分類方法を確認して独立実装した。

- [osm_water.py](https://github.com/circularsquare/trainsworld/blob/6b8ef91729e1e5812f3dcbaedd6c0a49fed6a84b/trainworld/pipeline/osm_water.py) の細密水域はGeofabrik PBFの面とOSM海岸線由来海域。日本設定は25mセル。小河川等を狭幅除去で意図的に省略し、覆蓋・季節水域等を選別する。線の幅を推測して水面へ変換しない。
- [world_water.py](https://github.com/circularsquare/trainsworld/blob/6b8ef91729e1e5812f3dcbaedd6c0a49fed6a84b/trainworld/pipeline/world_water.py) の粗密世界水域は海とNatural Earth大湖の1/60度セル。小河川の網羅性を保証しない。
- [clock.worker.ts](https://github.com/circularsquare/trainsworld/blob/6b8ef91729e1e5812f3dcbaedd6c0a49fed6a84b/trainworld/app/src/workers/clock.worker.ts) のマスク近傍取得と候補再評価を参考にする。ただし404や配列欠落を成功扱いする経路は採用しない。
- [NOTICE](https://github.com/circularsquare/trainsworld/blob/6b8ef91729e1e5812f3dcbaedd6c0a49fed6a84b/NOTICE.md) はコードAGPL-3.0とデータODbLを区別し、混在packの一部に追加制限も記載する。本変更はTWコード、都市pack、境界/人口データを収録・再配布しない。水域データ公開時には原典別の帰属・利用条件を維持する。

## 読込契約

細密入力は行境界u32と水域列ランu16のlittle-endian配列。格子は原点からの球面正距円筒座標、公開形式の半径6371008.8mを使用する。これは供給ラスタの座標変換であり、路線譜の線路距離や6m高さを変更しない。セルの東/北端は範囲外。寸法、有限値、緯経度範囲、配列長、行境界、ランの順序/重複/範囲を検証し、入力bufferとspecを内部コピーする。

世界マスクはTWW1とu32寸法、各行のLEB128境界差分を検証する。幅/高さと1度あたりセル数の整合、正規LEB128、偶数境界、厳密な境界増加、末尾余剰を確認。最大16MiB、境界数400万、1度182セルまでに制限する。粗密世界マスクの非水域セルはモデル陸地へ昇格させない。

`fetchWaterRange` はHTTPS URL・offset・length・期待SHA256を受け取り、正確な206/Content-Range、最大16MiBのstream長、SHA256を確認する。404、200へのRange無視、切詰め、過大body、hash不一致は例外。中断時はbody readerを取消し、古い結果を返さない。共有キャッシュや自動再試行はまだ実装していないので、道路側の転送状態に影響しない。

## 分類契約

- `model_water` / `model_land`: catalogで要求された細密格子が全て存在し、格子仕様と版が一致した地点。その解像度・選別規則でのモデル分類で、現実の未収録小河川がない証明ではない。使用した全source IDと最大セル幅を返す。`coverage=complete_model`が必要。`observations_only`は観測された水域セルだけを返し、空セルは`unknown/fine_incomplete`として陸地に昇格させない。coverage指定欠落は拒否する。
- `unknown/fine_missing`: 必要格子の欠落、spec違い、格子照会のnull。不足を重複する別格子や粗密水域で隠さない。
- `coarse_water`: 細密catalog範囲外で粗密マスクに水域がある。地上判定に使える精密境界の保証ではない。
- `unknown/outside_fine`: 細密範囲外で粗密水域の根拠もない。陸地へ変換しない。
- 重なった細密格子は水域側を採用する。同一地点で異なるrevisionの混合はエラー。catalogは1024格子まで。

catalog自体の真正性・入力PBFと境界の版整合・地域抽出の完全性はこのデコーダでは証明しない。呼出側が認証済みmanifestとモデルの採用条件を用意する必要がある。任意の矩形と空ランを渡しただけで実世界の陸地を認定してはならない。現stageのcoverage=[]をこのAPIだけで昇格させない。

## 接続前に残る作業

実データ配信、認証済みcatalog、保存するモデル版/出典、Rustとの格子入力契約、線路に沿う水域区間の計算、6m Hermite高さ、30km超・環状/分岐・-2/-3免除の共通評価が必要。previewとbuildは同じ候補と固定マスク版を使い、build直前にも検証する。未取得は既存小ウィンドウへ理由を返し、道路の参考追従を水域取得失敗で停止させない。未評価を許可へ読み替えず、全面停止する未完成gateも導入しない。

ローカル11試験・TypeScript型検査成功。境界、重複、欠落、版混合、壊れた行ラン、Range/hash/取消を検証した。実配信取得成功・WebView・建設拒否E2Eは未検証。2026-11-16のTW world.water取得試行はHTTP403で、空データとして扱っていない。

## 実データ読込の確認

固定関門composite `8057fde2778c8af3208c82f7306218382665aebbc8f2b672186de77b660a322e` の9部品についてmanifest/payloadのhashを確認し、属性で選別した観測面から25mのセル中心所属マスクを生成した。入力のsource issue 1件、非面の線/海岸線109観測、属性未確定等で除外した面2観測を記録し、coverageは空・geographic_completenessはunknownを維持した。これは旧固定試験compositeの読込確認で、全国stage更新ではない。

本番TypeScriptデコーダで全59,496セルを読み、水域15,691・Unknown43,805・model_land 0として生成側と一致した。2,620bytes、SHA256 `906aaa77adb59cb7baef692646564c59498f5d26e4c0789f177ea1f8122c3b92`。source issueや幅不明を、この成功で解決済みにしない。再現スクリプトはクラウドの `rosenfu-water-pack-artifacts/build-kanmon-mask-check.py` と `read-kanmon-mask-check.mjs`、結果は `kanmon-observation-raster-v1/`。生成物は本リポジトリへ収録・配信していない。

## 再現可能な観測マスク生成

`scripts/water-mask/build.py` は固定hashの既存compositeと全partのmanifest/payloadを検証し、観測面をセル中心で分類する。幅不明の線を面へ拡張しない。覆蓋・地下・高さ未確定・季節水域・basin属性の面は除外し、除外数、元のsource issue、原典hash/版/帰属を出力に保持する。`observations_only`専用で、非水域セルは常にUnknown。空セルを陸地とするcoverage証明は生成しない。25mセルより小さい水域や境界の正確性は保証しない。

原典集合と解釈規則 `observed-surface-centres-v2` から共通revisionを計算する。地域boundsだけでは版が変わらない。v2ではbasin矛盾属性も除外し、旧試作v1と混在させない。入力path/hash/サイズ、格子寸法、丸め後の緯経度範囲と面形状を検査し、既存出力は上書きしない。原典の完全性や全属性schemaの認証を代替するものではない。

```sh
python -m pip install -r scripts/water-mask/requirements.txt
python scripts/water-mask/build.py INPUT_COMPOSITE_DIRECTORY --pin COMPOSITE_SHA256 --output NEW_OUTPUT_DIRECTORY
python -m unittest discover -s scripts/water-mask -p 'test_*.py' -v
```

固定試験入力の富士山・関門・高松を再生成し、本番TypeScriptデコーダで全セルを照合した。富士山は水域0/Unknown25,988、関門は15,691/43,805、高松は15,847/33,881。全てmodel_landは0、source issueは各1件を保持した。これらは旧9part試験compositeであり、全国coverageや最新stageの反映を示さない。生成物はクラウド検証領域だけに保存し、HTTPS配信catalogへはまだ登録していない。

Python 8回帰（空入力、除外、版、hash/形状、上書き防止/上限、path、四境界、矛盾basin）を専用3 OS CIに追加した。公開済み読込コードの11試験に加えて生成器を検証する。

## 固定commitのHTTPS配信試験

`data/water-mask-trial-20261114/`へ3地点のマスクと原典metadataを登録する。
ODbL帰属と編集可能な元データの固定リンクは同ディレクトリのREADMEに記載。
`fetchFineWaterMask`は単独ファイルの200応答・hash・長さ・取消検証を経て復号し、await前にspecをコピーする。GitHub rawはContent-RangeをCORS公開しないため、独立した小ファイルを全体取得する。Range取得APIを200へ緩和するものではない。
catalogは1579bytes、SHA256 `07ec91cdbfeda5f0f2a8157fc303bf9824b5da084bcdc924c45f00ec92dc5bab`。
公開後は `node scripts/water-mask/check-delivery.mjs PUBLISHED_COMMIT_SHA` で、実HTTPS取得から全セル走査・水域/Unknown総数照合まで再現できる。
この登録は3地点の観測専用データであり、全国coverage、WebView動作、建設許可を追加しない。
