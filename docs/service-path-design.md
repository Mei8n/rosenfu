# Alignment上の運行系統: ServicePath設計案

状態: **設計提案。本文は実装完了を意味しない。** 2026-11-12。

## 目的と現状

路線・系統・線路を独立させ、線路先行で配置した駅を系統に使用する。系統は複数路線を経由でき、路線名と独立した名前を持つ。

schema 4のOperatingServiceはstart/tracks/stopsを持ち、stopsはTrack走行列の駅出現番号である。RoutePathはAlignment区間の現在の駅配置を動的展開する。ここに旧stops番号を直接流用すると途中駅追加で停車先がずれるため禁止する。

## 推奨schema 5形状

WorldDraftにservice_paths配列を追加する。既存OperatingServiceは変更せず、新方式ではそのtracksとstopsを空にし、対応するServicePathを唯一の経路・停車情報とする。sidecarのない系統は従来どおりTrack方式で検証する。

```text
ServicePath {
  service_id: ServiceId,
  next_leg_key: decimal-string u64,
  legs: [{ key: decimal-string u64, segment: RouteSegment }],
  stops: [StationOccurrenceRef]
}
StationOccurrenceRef = Start | OnLeg { leg_key, station_id }
```

### 安定した出現ID

leg keyはサービス内で正の整数、一意、永続、非再利用とする。配列位置でもStationId単独でもない。next_leg_keyは全keyより大きく、枯渇時は操作を失敗させる。既存のdecimal IDと同様にJSON数値精度問題を避ける。サービス外で参照する場合はServiceIdとの組にする。

各legは走行方向に解決し、始点を除外して途中駅と終点を生成する。サービス全体の始点だけStartで生成する。したがって隣接legの共有駅は直前legのOnLegに属し、二重に出現しない。逆方向でも同じ規則。同じ駅の再訪は異なるleg keyで識別できる。

stopsを現在の解決列に照合し、参照実在・厳密な走行順・重複なし・Startと最終出現の停車を検証する。浮動小数点の座標やpositionを出現IDにしない。leg keyは経路編集で既存legを保持する場合のみ保持し、別区間への置換では新規採番する。初期版は経路編集を未対応にしてもよい。

## 路線との独立と複数路線経由

路線から系統へ取り込む際、ユーザーが選択した区間のsegmentsをコピーして独立したleg keyを付ける。RouteIdへのライブ参照を権威情報にしない。路線の改名・所属駅変更・削除で系統は変化しない。

複数路線経由は選択した区間の連結として表現する。前区間の終点と次区間の始点のStationIdが同一であることを要求し、重複する境界駅は一つにする。経路候補が複数ならユーザーが選ぶ。単に座標が近い、交差している、駅名が同じ、という理由では接続しない。取り込み元の路線名等を将来表示する場合は由来情報であり、運行経路の根拠にしない。

現在StationPlacementはStationIdにつき一件であり、別Alignment間で同じ駅を共有するjunctionモデルはない。初期版は一本のAlignmentと、既存Trackで明示接続された混在経路を対象とする。複数路線経由の一般化と複数Alignment接続は別の課題である。

## 駅変更の意味

- 途中駅追加: 動的な通過候補一覧に出現するが、保存済みstopsは変更しない。新駅が既存駅の前に挿入されても停車先は不変。
- 新規系統作成: 「現時点の全駅停車」を明示選択可能にし、その時点の出現参照を保存する。将来の新駅にも自動停車する継続ポリシーは別機能。
- 改名: 同一StationIdなので停車設定に影響しない。
- 移動: 初期版では使用中の駅・placement・Alignment形状を暗黙に移動できないよう保護する。将来の専用編集では候補world全体を検証し、区間外移動・順序反転・接続破壊・停車参照消失は拒否する。停車順を自動で並べ替えて救済しない。spanのfrom/toを移すと区間自体が変わるため、妥当でも別途明示的な経路変更として扱う。
- 削除: Track/Alignment/leg端点/始点/停車駅の参照削除を拒否。初期実装ではサービスが通る全駅の削除も拒否する保守案を推奨。非停車の中間駅だけ削除許容する緩和は未決。
- 全変更は候補snapshotを検証してから原子的に適用し、失敗時はID counterを含め現world不変。

## 停車と運行停止の区別

本文のstopsは「停車駅指定」であり、一時停止・運休・運行中などの状態ではない。現段階で列車運行状態を捏造して追加しない。壊れた経路を読み込んで無言で運休扱いにする方式も採用しない。セーブ検証で不整合を拒否し、実行中worldは直前の正常状態を保持する。

将来の列車シミュレーションでは、工事中の経路変更等に対して明示的な運休状態を設けるか別途決定する。これはServicePath互換対応の必須範囲外。

## schema 4以前との互換

- schema 1〜4は旧形状で厳密に読み、service_paths: []を加えてschema 5へ移行。
- 旧サービスのID/start/tracks/stopsはそのまま保持し、駅列や停車番号を再計算しない。
- schema 5ではservice_paths配列を必須化。未知フィールドや旧schemaへの新フィールド混入を拒否。
- 既存legacy_v3が現在のOperatingService型を参照する点に注意。将来この型を変えるなら旧DTOを固定化する。
- 書き出しはschema 5。schema 4アプリでの読込は保証しない。既存バックアップ保存を維持する。
- sidecarのservice_id重複・孤児、旧tracks/stopsとの二重指定、未知leg key等を拒否する。

## 旧APIとUIのガード

legacy save_serviceがServicePath付きIDを更新する場合は明確に拒否し、sidecarを消して旧形式へ変換しない。新形式用保存APIは所有者・参照・接続・停車を候補全体で検証する。旧式UIの編集/保存ボタンは新式系統を対象にしない。

Track/Stationの削除ガードにServicePath参照を追加し、Alignment/placement編集削除にも同様の検証を置く。UIだけでなくcore/world検証でも防御する。サービス削除は本体とsidecarを原子的に消す。

## 段階実装

1. schema 5・migration・core検証・旧APIガード。
2. 一本のAlignment区間の系統作成、停車選択、保存読込。既存Track系統の挙動不変。
3. Track/Alignment混在、逆方向、複数路線から選んだ区間の連結。
4. 同一駅再訪、経路編集、junction。RoutePathの全体seenによる再訪禁止をそのままサービス全体に流用しない。

最初のUIでは未対応経路を理由つきで拒否する。仮Track生成による回避はしない。

## 最低限の試験

- 新駅を先頭側・途中に追加しても既存停車先が変わらない。
- 逆方向、leg境界の一意化、同駅往復の異なる出現。
- 重複/孤児sidecar、欠落配列、壊れたkey/stop、二重表現の拒否。
- schema 1〜4旧サービスの完全保持とschema 5往復一致。
- 旧APIによる新式上書き拒否、参照先削除拒否、失敗時snapshotとcounter不変。
- Route改名・所属駅変更・削除でコピー済み系統が不変。

## 未決事項

将来新駅の各停自動追加、非停車途中駅の削除許容、駅移動専用UI、複数Alignment駅/junctionモデル、経路編集時の停車引継ぎ、運休状態は未決。初期版で暗黙の挙動を足さない。
