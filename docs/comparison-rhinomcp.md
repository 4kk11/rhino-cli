# rhino-cli vs RhinoMCP 機能比較

`references/RhinoMCP`（= McNeel 公式 **Rhino MCP Platform**）と本リポジトリ rhino-cli を、実ソース（登録済み tool / handler / CLI subcommand）を根拠に MECE で比較する。README の宣伝文ではなく、両プロジェクトをサブエージェントで全ファイル走査して得た実装ベースの一覧に基づく。

> 注意: クローンされている RhinoMCP は旧 pip 版 `rhino_mcp`(Python + ソケット) **ではなく**、router ベースの新公式実装。比較対象を取り違えないこと。

---

## 0. 結論サマリ

- **思想が違う**: RhinoMCP は「AI に Rhino で**モデリング/設計させる**ための太い MCP ツール群（43 tool）+ マルチインスタンス運用基盤」。rhino-cli は「AI が Rhino を**起動・運用・解剖する**ための薄い JSON-RPC 基盤 + run_python エスケープハッチ」。
- **RhinoMCP だけにある主要領域**: Grasshopper(1/2) フル操作、マルチ Rhino インスタンス(slot) 管理、構造化された scene/object/selection/material tool、同梱の Claude Code エージェント群。
- **rhino-cli だけにある主要領域**: **.NET 型イントロスペクション/逆コンパイル（inspect-type/search-types/decompile-method）**、Rhino コマンドの probe（実プロンプト取得）、パネル WebView への JS 注入、プロセス launch/shutdown/screenshot（macOS）。
- **共通の核**: JSON-RPC 系プロトコル、Python 実行、Rhino コマンド実行、コマンド一覧、新規モデル作成、ビューポート画像取得。

---

## 1. アーキテクチャ / トランスポート

| 観点 | rhino-cli | RhinoMCP (Platform) |
|---|---|---|
| AI クライアント実体 | Rust 製 CLI バイナリ（`src/main.rs`） | router 実行ファイル（C# / macOS は NativeAOT） |
| AI への接続形態 | CLI を直接実行（サブコマンド / `call <method>`） | **MCP (stdio, JSON-RPC 2.0)** を MCP クライアントが起動 |
| Rhino 側 | RhinoCliPlugin（`RhinoCli.Server` ライブラリ上の handler） | RhMcp plugin（Kestrel HTTP MCP サーバを slot ごとに起動） |
| 中間層 | なし（CLI が直接 plugin に TCP 接続） | **router**（AI↔router は stdio、router↔各 Rhino は HTTP でプロキシ） |
| プロトコル | JSON-RPC 2.0（改行区切り JSON over TCP） | JSON-RPC 2.0（stdio MCP + HTTP） |
| デフォルト port | 50061（`--port`/env で可変、`plugin set-port`） | 子 Rhino HTTP は 10500〜（slot ごと自動採番） |
| 状態管理 | ステートレス（毎回接続） | SQLite で slot 状態を永続化、複数 router 並行可 |

要点: RhinoMCP は **router を挟んだ MCP プラットフォーム**で、複数 Rhino を束ねる前提。rhino-cli は **1 CLI → 1 plugin の直結 RPC**でプロセス管理は CLI 側に持つ。

---

## 2. 共通機能（双方にある）

| 機能 | rhino-cli | RhinoMCP | 備考 |
|---|---|---|---|
| Python 実行 | `rhino.run_python`（CLI `run-python` 相当） | `run_python` | 双方の万能口 |
| Rhino コマンド実行 | `rhino.run_script`（CLI `run-script`） | `run_command` | コマンド文字列を実行 |
| コマンド名一覧 | `rhino.list_commands`（CLI `list-commands`） | `get_commands` | パターン絞り込み対応 |
| 新規モデル作成 | `rhino.new_model`（CLI `new-model`） | （`open_doc` で clear / spawn 時に新規 doc） | rhino-cli は専用 handler |
| ビューポート画像取得 | `rhino.capture_viewport`（CLI `capture-viewport`） | `get_viewport_image` | PNG/JPG をAIに返す |
| 死活確認 / 自己記述 | `system.ping` / `rpc.capabilities` | （MCP の tools/list） | 経路は異なるが目的は同じ |

---

## 3. rhino-cli 固有（RhinoMCP に無い）

| 機能 | 実体 | RhinoMCP に無い理由 / 状況 |
|---|---|---|
| **.NET 型インスペクション** | `rhino.inspect_type`（CLI `inspect-type`） | RhinoCommon 等の型をリフレクションで解剖。RhinoMCP に相当 tool 無し |
| **.NET 型/メンバ検索** | `rhino.search_types`（CLI `search-types`） | ロード済みアセンブリを横断検索。相当無し |
| **メソッド逆コンパイル** | `rhino.decompile_method`（CLI `decompile-method`、ICSharpCode.Decompiler） | IL→C# 逆コンパイル。相当無し |
| Rhino コマンド probe | `rhino.probe_command`（CLI `probe-command`） | コマンドを起動→即キャンセルし最初のプロンプト/オプションを取得。background thread で Escape 送出。相当無し |
| コマンド履歴取得/クリア | `rhino.command_history` / `rhino.clear_command_history`（CLI `history`） | 履歴コンソール読取。相当無し |
| パネル WebView へ JS 注入 | `rhino.execute_in_panel_webview`（CLI `execute-panel-js`） | Eto.Forms.WebView を GUID で辿り JS 実行。相当無し |
| Rhino プロセス起動 | CLI `launch`（macOS、`--restart` 等） | RhinoMCP は router の `spawn_slot` が担う（別アプローチ、後述 §10） |
| Rhino 終了 | CLI `shutdown`（macOS） | router は `close_slot`/`_router_quit_app`（別アプローチ） |
| readiness 待機 | CLI `wait-ready` | MCP は接続で代替 |
| 接続診断 | CLI `doctor`（ActiveDoc 有無まで判定） | 相当 tool 無し（crash 診断は router 内部にあり、§10） |
| OS レベル window screenshot | CLI `screenshot`（macOS `screencapture`） | viewport 画像はあるが OS window 撮影は無し |
| port 設定 | CLI `plugin set-port` / `show-config` | port は router が自動採番 |
| echo/hello スモーク | `rhino_cli.echo` / `rhino_cli.hello` | （任意 tool で代替） |

rhino-cli の固有価値は **「Rhino/.NET の API を AI が動的に発見・解剖する」**（型検索・逆コンパイル・コマンド probe）と **macOS プロセス/ウィンドウ制御**。

---

## 4. RhinoMCP 固有（rhino-cli に専用機能が無い）

### 4-1. Grasshopper 連携（最大のギャップ）

rhino-cli は **Grasshopper 非対応**。RhinoMCP は GH1（11 tool）+ GH2（12 tool）をフル装備。

| GH 機能 | RhinoMCP tool（g1_/g2_） |
|---|---|
| GH 起動 | `g{1,2}_start` |
| コンポーネント配置 | `g{1,2}_place_component` |
| スライダ配置 | `g{1,2}_place_slider` |
| 配線（単一/一括） | `g{1,2}_connect` / `g{1,2}_connect_many` |
| ライブラリ検索 | `g{1,2}_search_components` |
| コンポーネント仕様取得 | `g{1,2}_describe_component` |
| キャンバス構造取得 | `g{1,2}_get_canvas_graph` |
| ソルブ | `g1_solve_graph` / `g2_solve_canvas` |
| キャンバス全消去 | `g{1,2}_clear_canvas`（要 confirm） |
| 定義一括適用 | `g{1,2}_apply_graph`（配置+配線を1回で） |

### 4-2. ドキュメント / シーン / 選択の構造化 tool

| 機能 | RhinoMCP tool | rhino-cli での扱い |
|---|---|---|
| ドキュメントを開く | `open_doc` | `run_python` で代替 |
| ドキュメント保存 | `save_doc` | `run_python` で代替 |
| ドキュメントを閉じる | `close_doc` | `run_python` / `shutdown` で代替 |
| オブジェクト一覧（型/レイヤ/可視でフィルタ） | `list_objects` | `run_python` + result_expression で代替 |
| 選択取得 | `get_selection` | `run_python` で代替 |
| 選択設定 | `set_selection` | `run_python` で代替 |
| C# スクリプト実行 | `run_csharp` | **無し**（rhino-cli は Python のみ） |

### 4-3. カメラ / 表示 / マテリアル

| 機能 | RhinoMCP tool | rhino-cli での扱い |
|---|---|---|
| カメラ設定 | `set_camera` | `capture_viewport` の引数 or `run_python` |
| オブジェクトにズーム | `zoom_to_object` | `run_script`(_Zoom) / `run_python` |
| レイヤにズーム | `zoom_to_layer` | 同上 |
| レイヤのマテリアル設定 | `set_layer_material` | `run_python` で代替 |

### 4-4. マルチインスタンス（slot）管理 — §10 参照

`spawn_slot` / `close_slot` / `list_slots`（+ 内部 `_router_*`）。rhino-cli は単一 Rhino 直結で相当機能なし。

### 4-5. 同梱 Claude Code プラグイン（cc-plugin）

RhinoMCP は AI 側の利用パッケージまで同梱：
- **エージェント8種**: rhino-modeller / rhino-organizer / rhino-drafter / rhino-inspector / rhino-teacher / grasshopper-scripter / grasshopper-reviewer / grasshopper-teacher
- **コマンド5種**: snapshot / scene / launch-rhino / launch-rhinos / install-mcp
- **スキル2種**: launch-rhino / launch-rhinos

rhino-cli にはこの種の同梱エージェント/コマンド資産は無い。

---

## 5. 同目的・別アプローチ（粒度が違う領域）

| 目的 | rhino-cli | RhinoMCP | コメント |
|---|---|---|---|
| オブジェクト CRUD | `run_python`（汎用1口に集約） | `list_objects`+`run_python`+`run_command`（型ごと構造化 tool 群） | rhino-cli は「ハンドラ境界ポリシー」で意図的に handler 化しない |
| ファイル I/O | `run_python` | `open_doc`/`save_doc`/`close_doc` | RhinoMCP は即戦力、rhino-cli は記述要 |
| プロセス起動/終了 | CLI `launch`/`shutdown`（OS プロセス制御） | router `spawn_slot`/`close_slot`（slot 抽象 + 複数管理） | §10 |
| 画像確認 | `capture_viewport`(in-proc) + `screenshot`(OS window) | `get_viewport_image`(in-proc) | rhino-cli は OS window 撮影も可能 |
| API リファレンス | `inspect_type`/`search_types`/`decompile_method`（.NET 実体を解剖） | `g*_describe_component`（GH コンポ仕様のみ） | 対象が違う：.NET 型 vs GH コンポ |

---

## 10. マルチインスタンス / ライフサイクルの設計差（重要）

| 観点 | rhino-cli | RhinoMCP (Platform) |
|---|---|---|
| 管理単位 | 単一 Rhino プロセス（直結） | **slot**（複数 Rhino を抽象化） |
| 起動 | CLI `launch`（macOS） | `spawn_slot`（version 指定可、Win=プロセス/slot、mac=既存アプリに doc 追加） |
| 終了 | CLI `shutdown` | `close_slot`（adopted は拒否）/ `_router_quit_app` |
| 列挙 | `list-plugins`（到達可能 plugin） | `list_slots`（dead を間引き、外部起動 Rhino を adopt） |
| 既存 Rhino の取込 | （手動 port 設定） | ファイル announce による **自動 adopt**（動物名 ID 付与） |
| クラッシュ処理 | `doctor` で切り分け（人/AIが判断） | `RhinoCrashReportFinder` がログ解析しエラーに理由を付与 |
| 自動起動 | 明示 `launch` 必須 | slot 未指定 tool 呼び出しで **auto-spawn** |

RhinoMCP は「N 台の Rhino をオーケストレーションする運用基盤」、rhino-cli は「1 台を確実に起動・診断する道具」。スケール思想が異なる。

---

## 11. 総括

- **被り**は Python 実行・コマンド実行・コマンド一覧・新規モデル・viewport 画像に集約。ここは双方とも揃っている。
- **RhinoMCP の強み**: Grasshopper(1/2) 完全対応、構造化された scene/object/selection/material/camera tool、複数 Rhino の slot オーケストレーション、同梱 Claude Code エージェント/コマンド。→ *「AI に設計作業をさせる」総合プラットフォーム*。
- **rhino-cli の強み**: .NET 型の動的イントロスペクション/逆コンパイル、コマンド probe、パネル WebView 制御、macOS のプロセス/ウィンドウ制御、薄い自己記述 RPC 基盤。→ *「AI が Rhino/プラグインを解剖・運用・自動操作する」低レベル基盤*。
- **最大のギャップ**:
  - rhino-cli に無い: Grasshopper、マルチインスタンス管理、C# スクリプト実行、同梱エージェント資産。
  - RhinoMCP に無い: .NET 型検索/逆コンパイル、コマンド probe、パネル WebView JS 注入、OS window screenshot、ActiveDoc 診断（doctor）。
- **設計判断の差**: RhinoMCP は handler を厚く積んで AI の即戦力にする。rhino-cli は「run_python で書けるものは handler 化しない」境界ポリシーで薄く保ち、代わりに *API を発見する道具*（型解剖・probe）を厚くしている。両者は競合というより**補完的**（運用・解剖の rhino-cli / 設計・GH の RhinoMCP）。
</content>
