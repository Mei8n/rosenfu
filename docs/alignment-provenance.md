# 参照地図由来の線形と出典（schema 6）

参照地図をゲーム設備へ明示的にコピーする際、線形点列と出典snapshotを同時保存する。外部データは未信頼であり、source文字列をURLとして自動取得・自動リンク実行したりHTMLとして描画しない。

## 保存とAPI

`World.source_records[]` は検証時点のdataset ID/revision、graph/source JSONのSHA-256、source/source_date、license、attribution、conversion_format_versionを保持する。keyは小文字64hexのgraph hashとsource hashをコロンで結んだ129byte文字列。同一keyは同一metadataでなければ拒否する。

`World.alignment_provenance[]` は `{ alignment_id, source }`。sourceは `source_key`, `method` (`osm_segment_copy_v1` / `osm_way_slice_v1`), `osm_way_id`, `mode` (`rail` / `road`), `start_segment_index`, `start_fraction`, `end_segment_index`, `end_fraction` を持つ。両端は元wayの向きに対するsegment/fractionとして記録し、逆向きコピーも可能。segment indexはJS safe integer範囲、fractionは有限0〜1。空区間は拒否する。

`build_alignment_from_reference({name,points,sourceRecord,provenance})` が専用デスクトップAPI。既存Alignment型は変えず、geometry・sidecar・採番を同時に検証する。出典上限や16MiB保存上限に失敗した場合も一部だけ追加しない。

バックエンドは構造・参照・上限を検証するが、pointsが指定bundleのwayと一致することを独立には証明しない。呼出側は検証済bundleからコピーする必要がある。hashはファイル整合の識別情報であって真正性・地理精度・権利の保証ではない。

## 上限・現段階の制限

- source_records 最大1024件、alignment_provenance 最大10000件。
- source metadata＋provenanceの概算byte予算2MiB（固定構造の余裕を含む）。さらに保存JSON全体16MiBを専用build時に検査。
- dataset_id/revisionは各200 UTF-8 bytes、sourceは1000 bytes、source_dateは64 bytes以内。非空文字列とし制御文字・主要な不可視書式文字を拒否する。source_dateは任意の出典表示であり日付の真正性を証明しない。
- 現変換器は version 1、license `ODbL-1.0`、attribution `© OpenStreetMap contributors` の組だけ受け入れる。
- metadataは黙って短縮しない。旧bundleが表示import可能でもWorld保存上限を超えるなら敷設copyを拒否する。
- 保存済み由来付き線形は点編集・出典単独削除・別出典への差替えに未対応。名称変更は可能。将来の点編集/複製/分割は出典伝播と変更履歴を設計してから提供する。
- 線形削除時は対応provenanceも一緒に削除しなければならない。SourceRecordは残す。外部bundle非表示・除去・更新によって既存geometryとsource snapshotを変更しない。
- ゲーム設備の接続性は別モデル。OSMの地理的交差や近接を自動で線路接続にはしない。

## 互換性と権利に関する境界

schema 1〜5は厳密な旧DTOを通り、空の両sidecarを追加する。旧版に新fieldを混入したデータは拒否。schema 6では両sidecarが必須で、帰属情報を落として旧版へ書き戻す機能は提供しない。

この保存機能だけでODbLの義務を満たすとは断定しない。原XML・変換bundle・手順は別途保持する。公開配布時の派生データベース/Produced Workの分類、提供方法、表示、ライセンス義務は別の検討事項であり、法的適合性を保証する機能ではない。

## 現在のUI

線路一覧から建設済み線形を選ぶと、ワールド内の出典スナップショットを表示する。由来記録ありと「出典記録なし（手描き・旧データ等）」を区別し、記録なしを手描きと断定しない。参考バンドルを外しても保存済み出典表示は残る。表示文字列は200 code unitsまでで省略を明記し、外部sourceをリンクやHTMLにしない。固定OSM帰属リンクだけを提供する。

この段階のUIは参照表示だけで、専用build APIを呼ぶ操作、スナップ、way区間コピー、既存点編集は追加していない。由来記録が欠けている場合は警告し、検証済みと表示しない。Windows実機表示は未確認。
