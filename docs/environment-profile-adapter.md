# 環境判定と区間別Hermiteの接続 checkpoint

基点は `feat/built-anchor-height-edit` の `219d805567b3871f150be0bd1d8b173246b47644`。
道路実装側の独立した環境判定モジュールを持ち込み、`assess_profile_alignment_environment` を公開する。
これはAPI接続検証用であり、取得UI/保存/着工コマンド全体を統合した版ではない。

各アンカー区間の長さで正規化位置を求め、`is_flat_over_position(from,to)` と
`height_at_position_m` で平坦かつ高さ0mを確認する。`section_profiles` があれば
`vertical_profile=None` でも検査を省略しない。同じ始終高さでも非ゼロ接線なら平坦ではない。
両profileがない場合だけ区間のInfrastructureLevelを参照する。高さ倍率・費用は追加しない。

道路側へ統合するときはsession/environment.rsのassessmentを
`rosenfu_core::world::assess_profile_alignment_environment(a, data)` に置換する。
道路側のWorldDraft.environment/保存/候補検証経路も同時に保持する必要がある。
このcheckpointにはそれらのアプリ接続を追加していない。

水域の距離・地上禁止判定はまだ構造レベルによるもの。斜面途中の実高さ別水域判定、
3-port分岐の全経路検査、縦断編集/分岐/着工の全変更経路への適用は未統合。
したがって、このAPI checkpointだけをもって勾配版の水域建設安全性を検証済みとしてはならない。
