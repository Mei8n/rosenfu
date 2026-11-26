# 0.2アルファ: バスとトラムの道路上設置

バスと道路上のトラムはα0.2の要件。バスを1.0以降へ送る旧案は撤回した。α0.2開発統合には、試験用道路データでバス・トラムを検証する機能が入っている。**実道路でのバス・トラム経路の一般利用や全国の通行性は未検証**。以下は残りの実装・受入も含む計画。

## 版と責務の境界

- **0.1**: 線路数1/2/4、7段階高さ・勾配、分岐、道路/水域判定・踏切、参照保護・保存。道路追従は線形作成の補助であり、道路上を走るバス/併用軌道の意味をまだ持たない。
- **0.2**: バスの道路経路・停留所とトラムの道路上併用軌道を設置・編集・撤去・保存できるようにし、同段階の共通時計・簡易運行・基礎需要へつなぐ。トラム専用軌道との切替を区間属性として扱う方針案。道路上設置という本人要件と、具体的なデータ設計案は区別する。
- **後続**: 詳細な道路交通混雑、信号制御、自動車一台単位の走行、詳細経営は今回の追加指定に含めない。実装順は依存に従い、日付は保証しない。

## 共通道路基盤と分離

全国道路は既存OpenFreeMapベクタータイルを必要範囲で自動取得・キャッシュする方針。独自全国パックの生成/配布を前提にしない。0.1の建設補助・踏切と0.2の道路交通は、同じ取得・座標・出典・版・欠測状態を再利用する。提供条件と帰属表示を維持し、TrainsWorldは方法の参考に留めコードを直接流用しない。

地図描画用タイルは完全な車両通行グラフとは限らない。以下は用途別に検証し、道路が見えるだけで通行可にしない。

- 車道/歩道/階段、一方通行、進入制限、橋・トンネル・高さ、タイル境界の接続。属性不明は不明のまま保持し、徒歩専用をバスへ無条件開放しない。
- 道路追従の近接補間と、実際の走行経路の接続判定を分離する。見た目の交差・高さ違い・未接続タイルを物理接続と推測しない。
- バスは鉄道線路そのものを作らず道路経路を参照する。トラムは道路への対応と鉄道の線路設備を両方保持する。交通モード別の経路制約を鉄道路線へ混ぜない。
- 取得失敗/欠測/対象外を「道路なし」へ変換しない。未キャッシュ範囲には通信が必要。既存ワールドの編集・保存を、未取得の全国完全性の要求で一律に停止させない。
- 背景OSM表示を維持し、OFMとの位置ずれは実測する。道路取得の採用だけで背景変更を行わない。

## 実装順と受入

1. 0.1の道路基盤・交差判定を固め、道路グラフに必要な不足属性を洗い出す。通行制約やタイル境界が証明できない箇所を診断できるようにする。
2. バス経路/停留所、トラム併用/専用区間の設置編集を独立に実装する。道路との重なりは横断とは異なるため、併用区間を大量の踏切として生成しない。交差点での正確な扱いは実装前に決める。
3. 運行・旅客グラフへ交通モード別に接続する。固定小地域ODの試験で、バス/トラムによるアクセス・乗換候補と鉄道未利用の選択肢を比較する。未実装の乗客数・収支を捏造しない。
4. 作成→修正→撤去→Undo/Redo→保存再開、並行道路・一方通行・高架下・タイル境界・通信失敗を回帰試験。道路データ更新で停留所や路線/系統参照が孤立した場合は勝手に再割当せず、理由と修正経路を示す。
5. 独立レビュー、自動試験・3 OS CIと実機受入を別々に記録する。未確認の全国通行性やオフライン完全対応を完成扱いにしない。

## 駅連絡・需要との整合

近隣1km以内の駅を手動選択する連絡通路・駅舎・統合は将来要件として維持するが、今回の0.2指定だけで全UIの完成時期を決めない。統合時は同一駅・同名、未統合時は別駅名を保ちつつ乗換/相乗効果を持たせ、利便性・待ち時間・満足度評価へ接続する。

交差ホームの同一駅group、物理線路の接続、旅客の歩行連絡は別の関係。同一駅だから別高さの軌道が接続する扱いにはしない。バス停・トラム停留所の鉄道駅アクセスも旅客側で扱い、現在の駅groupを壊さない。[需要・流動計画](demand-flow-plan.md)と同じ計算境界を使用する。

### Regional routing input: current release gate

The checked-in Tokyo-centre publication preserves exact OSM node IDs and node
order, but its v1 cell schema and publication pipeline omit the direction tags
and restriction relation membership needed to authorize road itineraries. Shared
IDs can establish geometric incidence only. The existing pack and display-only
OpenFreeMap sources must not be promoted to allowed road edges.

A regional pilot needs a distinct, pinned raw input (source URL, snapshot
timestamp/hash, certified polygon, extraction flags and ODbL attribution). Preserve
original IDs, way direction and access hierarchy, intersection layers, and all
applicable turn-restriction members (including via-way/conditional cases).
Reject paths through unresolved permissions, omitted restriction members, unknown
direction, or clipped boundaries; do not fill gaps by joining coordinates. OSM
source values document the snapshot, not a warranty of current legal access.
An unsupported restriction can block a pilot junction without disabling unrelated
world editing. The checked-in input currently has no routable real region.

The opt-in QA fixture is synthetic and visibly labeled. It exercises directed
path validation and the real rail construction transaction. Managed trams can
attach or detach the approved matching path; detaching does not move the rail.
Rail dependencies still block removal, and each supported tram edit is saved and
undoable atomically. Managed dedicated and shared-road trams can run on their
physical rails using the normal rail service model. The shared-road attachment
does not yet add mixed-traffic conflicts, intersection control or a separate
road running model for trams.

### 王子駅前〜飛鳥山: データ確認と未接続の検証基盤

初回候補は国道122号・明治通りの王子駅前〜飛鳥山の途中にある
地上の併用区間。交差点、駅前の分岐・渡り線、JR高架周辺は初回対象外。
[東京都交通局の支出資料](https://www.kotsu.metro.tokyo.jp/about/koukai/koukin/pdf/koukin_r0604_01.pdf)
は両停留場間に併用軌道部があることを確認できる一次資料。
[東京都議会の記録](https://www.gikai.metro.tokyo.lg.jp/record/police-fire-fighting/2001-04.html)
も明治通りと飛鳥山交差点を区別できる。これらは区間の存在を示すが、
道路の各方向・車線・バス停位置・進入条件を網羅する台帳ではない。

2026-11-20に [OSM map API の限定範囲](https://api.openstreetmap.org/api/0.6/map?bbox=139.7325,35.7479,139.7405,35.7550)
を開発時に一度取得した。実データでは車道と軌道は別の方向別 way。
周辺には時刻条件付きアクセス（way 27867901）、via-way のUターン規制
（relation 12714546/12714547等）がある。地図の一致やタグの不在を通行可とはしない。
照合した応答の SHA-256 は
`7b18c0cc76de0b75015d25defb1e274a56e5d65d7d32ffca9fa7a363c8c32bda`。
当該 XML は個人名等を含むため同梱しない。データ配布時は個人メタデータを除いた
必要範囲だけを抽出し、出典、版、抽出範囲、ハッシュを固定した独立アセットにする。
[OSMのデータ条件](https://www.openstreetmap.org/copyright)に従い、OSMへの帰属表示と
ODbLを保持する。公開 API はアプリの実行時には呼ばない。

[JARTICの規制情報](https://www.jartic.or.jp/service/opendata/)の公開と
[再利用条件](https://www.jartic.or.jp/d/opendata/riyou_kiyaku.pdf)は確認済み。
出典・利用日・加工表示および第三者権利の確認が必要。
[2026-11-01公表の作成基準日一覧](https://www.jartic.or.jp/d/opendata/202611010000/typeD_kisei_103_UpdateList.pdf)に
よると、警視庁のデータは2026年9月末時点で、標準フォーマットをK_2.1に形式変換したもの。
今回は対象車線・方向に対応付けられる規制レコードや現況を確定できていない。
[データ仕様書](https://www.jartic.or.jp/d/opendata/typeD_kisei_73_k_2.0.pdf)
にも収録漏れや更新差の注意があり、欠落は許可の根拠にしない。
配布可能な正式アセット、再現可能な対応表、現行の標識・管理資料による確認は次段階。
実在区間の通行フラグは設定していない。

「道路交通」の地域一覧には、ネイティブが照合した同梱データだけを、地域・出所・版・
帰属付きで表示する。現在は追加できる承認済み地域がないので、一覧エラーとは区別
して表示する。将来追加した地域のバスは承認済み乗降位置から起終点を選ぶ。
方向付き辺に沿って接続を探索し、進入方向・停止位置を変える場合も同じ検証を通す。
2停留所の系統の設定変更が対象で、削除しても他系統が参照し得る停留所は残す。
既存の架空QAは実在道路と別枠のまま。

ネイティブの実在道路インポートは、リリース時にレビュー済みのIDとSHA-256を
固定した同梱JSONだけを受け付ける。現在の登録は **0件**。IPCは対象IDと
ワールド照合トークンだけを受け取り、パス、取得URL、道路データ、許可表を
クライアントから受け取らない。既存グラフへの上書きも行わない。
架空QAは従来の明示操作で同じ合成グラフだけを利用でき、実在道路扱いにしない。
通行を許可しない地図由来の参考データは、そのまま参考用として読める。

将来候補の版1アセットは、安定ID順に並べた地上有向グラフと、モード別の
正の遷移（進入辺・退出辺）、バス停を置ける有向辺と割合を保持する。
経路検索と建設・編集・保存・読込で同じグラフと許可表を検証する。
未記載の遷移、逆方向（独立の承認済み辺がない場合）、Uターン、地上不明を
許可しない。停車許可は走行許可と別で、軌道の併用可否もバスと共有しない。
未対応の時間条件・連続した複数辺にまたがる規制がある場合は、規制の影響を
排除できない辺を候補に入れない。地図上の一致は通行許可ではない。

登録追加には、別途元資料・取得日・出典/権利・区域・完全な規制確認・バス停
位置・併用軌道と道路との対応を記録し、配布対象のバイト列とSHA-256をレビュー
する必要がある。グラフの検証成功は現地資料の正しさや配布権の証明ではない。
セーブ形式やUIは今回変更せず、承認された実在区間なしで機能を開放しない。

#### 同じスナップショットで再確認する

元の限定 XML が手元にあるときは
`python scripts/road-pack/audit_oji_pilot.py /path/to/oji.osm` を実行する。
最大8 MiB、上記SHA-256・OSM 0.6・指定範囲と完全一致した入力だけを読む。
音無橋の信号・軌道交差に入る前の道路 way 209826275（約51.3 m）と、
軌道 way 343234687（約54.1 m）の別々の断片を検証する。
各断片では元のノード順、一方通行タグ、境界との余裕を確認し、
別wayと共用するノード、関係する取得済み規制、タグ付きノード、
条件付きタグ、高さ関連の非既定タグがあれば中断する。
取得データにない規制が存在しないとは判断しない。

出力は出典、対象範囲、way ID、概算長だけの監査用情報で、
`routing_authorized: false` 固定。車道と軌道の同一レーン・現行の通行許可、
完全な道路網や隣接区間への接続を証明するものではない。
元XMLや個人メタデータを保存・配布する処理ではなく、アプリからも呼ばない。

### 共通時計・固定ODの受入範囲

- バスは方向付き道路経路と停留所を検証し、鉄道と同じ運行時計で走る。旅客の停留所ID・系統IDは `bus:` 名前空間で鉄道と分離する。併用・専用トラムは実際に敷設した軌道を通常の鉄道系統として運行設定し、共通時計と固定ODに参加する。併用区間で自動車やバスと競合する仕組みではない。
- 「都市」の固定OD試算で、任意の駅（トラム停留場を含む）とバス停の組を一つ、徒歩1〜60分で指定できる。指定した場合に限り試算内で両方向の乗換候補を追加し、時間・乗換回数と他の移動手段を比較する。近い駅や同名の地点を勝手につながない。実在の徒歩経路・通行可否、駅の統合、乗換設備の建設や保存を行う機能ではない。ワールドには従来の出発地・到着地・人数・代替所要時間のみ記録する。
- 合成QAのバスを普通鉄道・専用トラム・併用トラムそれぞれと同時に動かす試験で、明示の徒歩リンクがない場合の未配分と、指定時の乗換・徒歩時間・人数・ワールド非変更を検査する。実地域の車両通行認証、全国データやWindows実機は別の受入項目。

### ゲーム内バスの運行方向と途中停留所

通常のバスは「新しいバス系統」の前に、往復または片道を選べる。往復は往路と復路の両方を確認できる場合だけ確定する。片道は起点から終点だけを運行し、別途「地図データの一方通行に従う」を選べる。復路が見つからなくても往復を片道に自動変更しない。現在の経路を引き直すときは保存済みの運行方向を保持する。

途中停留所の編集一覧は配置した順ではなく往路の停車順で表示する。停留所を移動しても識別子は変わらず、運行方向や道路根拠も変更しない。
