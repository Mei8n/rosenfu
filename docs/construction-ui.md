# 地図主体の建設 UI（試作）

## 操作

- 地図主体の画面、上部の保存・読込、左側の高密度な操作パネルとツールタブで構成する。ライト／ダークを切り替えられる。
- 地図左上の「地図・表示設定」に背景選択、ズーム・座標移動、線の太さ・地図コントラスト・パネル幅・文字サイズをまとめた。通常背景はオンラインの OpenStreetMap。地理院地図と自作サンプル（オフライン）にも切り替えられる。オンライン時は現在の表示範囲だけを取得し、取得失敗でも建設・編集・ローカル保存は続けられる。
- 最初のツールは「線路」で、選択すると標準マウスカーソルが十字になり、そのまま描画できる。線路名と直線・曲線を選び、最初のクリックで始点、次のクリックで次点を計画へ追加する。カーソルまでの区間がプレビューされる。専用画像カーソルは使わない。
- 地図の短いクリックで点を追加、ドラッグで移動。計画は青い線と四角い制御点で表示され、「計画を着工」で現在の単一線形を確定する。「1点戻す」／「やり直す」で未着工の点を Undo/Redo できる。Backspace は末尾の点を戻し、Esc は下書きを維持して移動モードに戻る。
- 「駅」では建設済み線路を選び、ホーム長・幅をメートルで指定する。地図で線路付近をクリック、または位置スライダーで位置を選んでプレビューし、「ホームを配置」で確定する。
- 駅は縮小時（ZL14以下）には点、詳細表示（ZL15以上）では保存された線形に沿う平端のホーム帯で描く。既存の単独駅は詳細表示時に長方形マーカーで表示する。
- 駅を選ぶとホーム長・幅を変更できる。寸法変更を適用後、ワールドを保存する。
- 旧ワールドの単独駅配置や駅間接続は、各ツール下部の補助操作に残している。

## 今回の限界

OSM の実際の道路・線路へのスナップ、既存施設トレース、分岐、勾配、編成両数や列車長によるホーム収容判定は未実装。線形建設と既存の運行経路モデルは分離しており、経路・系統で使用する駅間接続は補助操作から手動で定義する。線形データは特定の地図プロバイダーに依存しない。

ホーム寸法は実寸のため、低ズームでは小さく見える。詳細確認にはズームを上げる。ホームの全長が線形より長い場合はバックエンドが拒否する。

UI は trainsworld の地図主体の操作構成を参考にした独自の再編であり、画面の完全再現ではない。実画面確認の可否とテスト結果は作業報告を参照。


## 後続の要望

この文書は現行試作の操作説明。縮小時の点表示、20m級換算の駅長、設備全般のUndo/Redo、地名候補などの追加要望は[追加設計メモ](construction-and-service-planning.md)を参照する。現在の操作を最終仕様とみなさない。

## Compact construction desk (2026-11-13)

- Light/dark compact left inspector, 38px top bar; display controls change rail thickness, contrast, panel width and text size. These display preferences now persist locally (see below).
- Selecting the track tool immediately activates crosshair drawing. First click sets an anchor; pointer movement previews the next segment; another click appends the next plan anchor. Drag still pans.
- Plan anchors are square and planned geometry is blue. Undo/Redo operate on unbuilt points. New input clears redo. Escape pauses drawing without discarding the plan.
- `計画を着工` commits the current single alignment. This is not a multi-asset construction queue. Planned stations, multiple parallel tracks and budget accounting are not implemented.
- The cursor-adjacent panel shows sampled path length and an approximate minimum radius from consecutive sampled triples. The radius is informational, not a validated engineering/design-speed calculation. Speed, cost multiplier, construction cost and maintenance remain explicitly uncalculated. Consecutive duplicate points block construction; terrain, minimum radius, reversals and OSM snapping are not certified by the basic validation.
- Display preferences are stored locally, separate from World saves. The existing route/service editors retain their independent roles. Route candidate previews can now be explicitly saved as new RoutePath routes; see the schema 4 section below.

## RoutePath 路線の UI（schema 4）

起終点候補の所属駅を確認して「候補を新規路線へ適用」で線形を含む路線と所属駅除外をワールドへ適用できる。ファイルへの反映は上部の「保存」で行う。保存失敗時は候補を残す。既存の新方式路線を選ぶと所属駅と物理区間を表示し、旧 Track 列エディターを無効化する。新方式の既存路線の変更 UI はまだ未実装。路線に参照される駅・Track は削除対象にできない。系統の停車指定とは独立した所属駅情報である。

## 駅の縮尺表示と20m級換算

ZL14以下では駅を点、ZL15以上では線形に沿うホーム帯で表示する。表示切替は保存されるホーム寸法を変更しない。単独の旧駅は詳細表示でも長方形マーカー。新規・既存駅のホーム長にはメートル入力と「20m級換算（両相当）」入力を併記し、1両相当＝20mとして双方向換算する。小数値を認め、整数両数へ丸めない。既存の231.5mは11.575両相当のまま扱う。内部値・保存・APIはメートルのまま。現行エンジンの20〜1000m範囲（換算1〜50両相当）は維持するが、Trainsworldの3〜20両制限は導入しない。この換算は長さの目安であり、実車長・余裕長・停止位置・収容力の判定ではない。

## 駅設置の限定Undo/Redo

- `Ctrl+Z` / `Ctrl+Shift+Z`（MacはCmdも可）または駅パネルのボタン。入力欄内では通常のテキスト編集Undoを優先する。
- 対象はこのセッションで新規設置したゲーム駅の履歴、最大50件のみ。「未保存ならどの設備でも取り消せる」機能ではない。参考駅パックは対象外。
- 線路ツールで未着工点のUndo/Redo履歴があるときはそちらを優先。それ以外は駅設置履歴を使う。
- 保存・World読込の成功、駅名/ホーム寸法の変更成功、手動駅削除の成功で駅設置履歴を終了する。保存/読込失敗では履歴を消さない。保存済み・読込済みの旧駅をUndoで消すことはできない。
- Undoは対象駅の状態が設置時と同じか確認し、Track・路線・系統などの参照がある場合は拒否する。既存のバックエンド削除ガードも併用し、連鎖削除はしない。理由を表示し履歴は保持する。
- 未適用の路線・系統・駅名・ホーム寸法などの編集、ホーム位置プレビュー、確認ダイアログがある場合は、先に適用/解除を求める。
- Redoは元の名前・位置・線形・寸法で再設置するため駅IDは新規発行。World全体のスナップショットを巻き戻さず、他の駅や設備を復元/上書きしない。操作失敗時は履歴を保持して再試行可能。新しい駅の設置はRedo履歴を破棄する。
- 名称変更・寸法変更・既存駅削除・線路/路線/系統編集そのもののUndo/Redo、保存をまたぐ履歴永続化は未対応。


## 表示設定の端末内保存

背景（OSM/地理院/オフラインサンプル）、ライト/ダーク、線幅、地図コントラスト、パネル幅、文字サイズは変更時にこの端末のlocalStorageへ自動保存し、次回起動で復元する。World・共有セーブには混ぜず、ネット送信しない。「表示設定を既定に戻す」で既定値へ戻し、この設定キーだけを削除する。

保存形式はversion 1。旧版（版なし/0）と欠損フィールドは既定値で補い、不正値・未知版・壊れたJSON・4KiB超・ストレージ拒否は既定値へフォールバックする。復元失敗時は保存済みデータを自動上書きしない。保存が拒否された場合も現在セッションの表示変更は使え、保存失敗は地図上にも表示する。リセットの削除失敗時は次回に以前の設定が戻る可能性を明示する。

現在の範囲は線幅2〜8、コントラスト70〜140%、パネル幅260〜400px、文字サイズ11〜16px（整数）。オリジン/ブラウザプロファイルごとの保存で、開発プレビューと配布アプリ間の共有は保証しない。他画面からの変更は現在画面へ即時同期しない。サイトデータ削除等で失われる場合がある。ストレージが使えなくてもゲーム編集・World保存は継続できる。

背景は初回レンダー前に同期復元する。保存済みサンプル選択では最初から画像タイルを生成しない。背景未保存の旧設定・初回・不正設定・読込拒否では既定のOSMを使用する。明示選択して保存したオンライン背景は次回起動でも取得を再開する。既定戻しもOSMになる。サンプルは模式図でありオフライン実地図ではない。画像要素の生成抑止を自動テストで検証するが、実機WebViewの通信計測は未実施。

### 駅詳細の名前編集
ゲーム駅を選択後、詳細の駅名をダブルクリック、または「選択駅の名前を編集」ボタン（Tab/Enterでも操作可）で既存の名前入力にフォーカスして全選択する。Enterは従来の駅名適用、Escは未適用の名前だけを元へ戻して編集ボタンへ戻る。失敗時は入力を保持する。処理中は開始・取消できず、参考駅は編集対象外。従来の入力欄もそのまま使え、入力中のCtrl+Zはテキスト編集のまま。
