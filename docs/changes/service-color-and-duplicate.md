# 系統色と複製（開発候補）

- `set_service_color(serviceId,color,expectedColor)` は `#RRGGBB` を検証し大文字化。現在色との一致確認後に原子的適用。未設定はnull、不在行を勝手に生成しない。
- `duplicate_service(serviceId,expectedPath,expectedName)` は新IDを割り当て、物理経路・停車・明示binding・設定色をコピー。未知binding/未設定色はそのまま未知/未設定。元系統は変更しない。
- 複製の名前は元名に「 のコピー」を付ける（80文字上限内に収める）。新しいIDの系統名・停車・色は元と独立編集。
- schema19保存。schema18は色未設定として読込、1〜17は非対応を維持。19は旧18専用アプリで開けない。読込だけで元ファイルを変えない。
- GUI/全量/3OS CIは統合担当の検証待ち。表示UIは別PRと一体統合する。
