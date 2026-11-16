# 全国路線網へ向けた保存・根拠容量の分離設計

調査基準: Rosenfu dev `dee00e22` (schema25)。これは設計と計測基盤のcheckpointであり、全国容量対応の完成ではない。既存セーブ・上限・World判定をこの段階では変更しない。

## 現行上限の目的と詰まり方

- `ofm_evidence/mod.rs`: 単回decodeは16 tiles / 8 MiB、単tile5 MiB。壊れたMVT・過大要求への時間/メモリ防護。
- `ofm_evidence/store.rs::with_additions`: 同じ要求制限を永続World全体へ適用し、追加時に全candidateを再decodeする。raw共有でもbindingごとにbytesを課金。全国へ路線を広げると単回操作が小さくても限界になる。
- `world/road_path.rs`: 建設済みprovenance＋未着工OFM経路の合計32。descriptor再生成検査の総仕事量を制限する初期安全枠。
- `storage/evidence.rs` と `storage/lib.rs`: 全rawをbase64付き単一JSON envelopeへ入れ16 MiBで拒否。原子保存は守るが、rawの約4/3倍化、全体encode/decode、backupとautosaveの重複が生じる。

これらを単純に無制限化しない。全国に線路を張ることと、全国の背景タイルを全部メモリへ保持することは別である。

## TrainsWorldとの比較（固定ソース調査）

参照版: `circularsquare/trainsworld` commit `6b8ef91729e1e5812f3dcbaedd6c0a49fed6a84b`。

- [`saveFile.ts`](https://github.com/circularsquare/trainsworld/blob/6b8ef91729e1e5812f3dcbaedd6c0a49fed6a84b/trainworld/app/src/workers/saveFile.ts) はJSONヘッダ＋バイナリ線路網(TWG1/TWT6)をgzipし、workerで処理、IndexedDBのslot/backupへ保存する。ゲーム入力と外部地理資産を分ける設計であり、RosenfuのOFM raw根拠内包契約と同じではない。
- [`gzip.ts`](https://github.com/circularsquare/trainsworld/blob/6b8ef91729e1e5812f3dcbaedd6c0a49fed6a84b/trainworld/app/src/workers/gzip.ts) は大きい圧縮/展開をUI threadから分離する。サイズ指定可能なAPIはあるが既定InfinityをRosenfuへ模倣しない。
- [`track/save.rs`](https://github.com/circularsquare/trainsworld/blob/6b8ef91729e1e5812f3dcbaedd6c0a49fed6a84b/trainworld/sim/src/track/save.rs) は線路網のID/index等をバイナリ化する。全国規模の性能保証や無制限保存の証明としては扱わない。

これは構造の比較であり、AGPLコードを移植しない。Rosenfuの『由来線は根拠rawから再生成検証する』契約を捨ててTWと同じ軽さになったとは言わない。

## 安全な実装段階

1. **計測（本checkpoint）**: immutable EvidenceStoreのbinding数、unique raw数/bytes、binding課金bytes、base64 payload bytesを別々に取得。JSON全体サイズやheap量とは区別する。既存制限は全保持。
2. **局所要求と永続規模の分離**: 1操作16tile/8MiB等を保持し、既検証immutable blobをcontent hash＋座標/版/decoder ruleで索引化。追加分のみ検証し、既存cacheの信頼は外部DTOではなく内部生成物に限る。mixed revisionを無条件に許可せず、各経路の全scopeが同一契約を満たすことを検証する。
3. **分割保存**: rawをcontent-addressed immutable blobとして保存、World世代manifestはhashを参照。blob一時書込→sync→hash検証→公開→manifest CAS/atomic swapの順。失敗時旧manifestとbackupは完全復元可能。通常World JSONだけでexportしない。portable exportは必須blobをまとめ、全entry長/総展開量/パスを検証する。
4. **root pinとGC**: 現WorldだけでなくUndo/Redo、autosave、backup、別slot、進行中transaction、exportをrootとして参照を保持。lease/世代を確定してから未参照だけ回収。現WorldだけでGCするとUndoの根拠を失うため先に導入しない。
5. **局所再検証とcache予算**: descriptor内容＋required binding hash集合＋decoder版のキーで再生成結果をmemoize。境界候補は全scope維持、road subsetで曖昧性を隠さない。decoded tileはbounded LRU、rawはディスク保持。経路数の固定32を外す前に変更対象/依存閉包へ検証仕事を局所化し、総仕事量超過は理由付き中断・元World保持とする。
6. **UI/IPC**: 全raw/全Worldを毎回JSONで送り直さない。小さい差分/opaque token＋世代lease、進捗/取消、同時処理上限。成功mutation後の応答破棄でUIが取り残されない契約は維持。

## 受入ゲート

- 17以上の離れたtile、33以上の独立経路を増築でき、追加操作の仕事量が全履歴量に比例しないことを実測。
- 100 / 1,000 / 10,000経路で追加・Undo・save/load・peak RSS・bytes・disk I/Oを測定し、全国相当の複数地域コーパスでも評価。数値目標は測定前に成功扱いしない。
- 同一raw別binding、複数版、境界曖昧、破損/欠落blob、symlink/path traversal、容量枯渇、保存各段の中断、競合、backup復旧、GCとUndo/別slot競合を検証。
- アーカイブ展開上限とディスク予算は残す。上限に近い場合は保存前/操作前に実使用量と対策を示し、既存データを削除しない。
- schema変更・旧版非対応は事後明示、旧ファイル非破壊。3OSと実機の大規模保存再開を終えるまで全国無制限を称さない。

## 現段階の実装

`EvidenceStore::usage()` は読み取り専用の容量計測。raw同SHA共有とbinding課金の差、scope/clone後の不変性を回帰する。保存分割・GC・上限拡張・UI接続は未実装。進行中の全国水域readerとは別branchで扱う。
