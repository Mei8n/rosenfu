# 路線候補・所属駅のプレビュー

## 現行範囲（schema 20）

物理経路は Alignment と明示的な端点接続のみ。旧 Track、`track_only`、旧 Route.tracks と旧保存 API は廃止する。探索とプレビューは読み取り専用で、設備や系統を勝手に作らない。

- `preview_route_candidates({ start, end })` は候補・状態・打切り情報・最短候補 index を返す。
- 同じ `preview_route_candidates({ start, end, viaStationIds })` の経由駅配列はクリックした順序を経由条件にする。内部では `preview_route_candidates_via` を使う。
- `preview_route_membership({ start, segments, excludedStations })` は物理経路と所属駅を検証する。
- `save_planned_route({ routeId, name, start, segments, excludedStations })` は明示保存する。新規だけ採番し、編集は路線 ID を維持する。

区間は `{ kind: "alignment", alignment_id, from, to }` または `{ kind: "connected_alignments", from, to, connections }`。RoutePath は `route_id`、順序付き `segments`、`member_stations` を持ち、すべての路線に対応する経路が必要。

## 探索・操作

地図上の駅をクリックして選ぶ。起点・終点だけを選んだ場合も、確定する物理経路上の中間駅を路線所属に含める。これは系統の自動作成・自動停車を意味しない。経由駅を順に選ぶと、その駅列を通る経路を探す。

状態は `no_path` / `unique` / `ambiguous` / `incomplete`。探索が完了した場合だけ物理距離に基づく最短候補を提示できる。同距離候補は決定的な順序で扱う。打切り候補から最短を断定しない。候補上限32、探索ステップ100,000、区間数256の制限があり、一般的な循環・反復経路の自動探索は未対応。

同一 Alignment 上の駅位置と明示接続から経路を構成する。地理的な交差・近接、同じ駅グループという理由だけで未接続線路を通さない。乗客の最短時間経路、転轍機、方向規制、他社線利用契約、容量を計算する機能ではない。

## 所属・保存・保護

所属駅の除外は設備撤去や停車方針ではない。物理経路を保持し、起終点は所属を維持する。保存区間内に駅が増えても既存路線の所属へ黙って追加しない。ワールドが変わった場合は古い候補を再検証する。無効・切断・過大な経路は適用しない。

系統との関連付けは [運行系統](operating-services.md) の独立した検証対象。路線を保存しただけで系統・停車・列車運行を作らない。

保存形式は schema 20 のみ。schema 1〜19 は非対応を明示して拒否し、読込だけでは元ファイルを変更しない。旧 Track 形式の偽装も拒否する。自動移行・旧版との往復互換はない。

## 検証

core の経路・経由駅・最短候補・関連付け試験、storage の現形式往復・旧形式拒否・元ファイル保護、session の失敗原子性を確認する。UI の自動試験とネイティブ実機の受入は分け、CI 成功を実機合格にしない。
