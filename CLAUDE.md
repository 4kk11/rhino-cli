# rhino-cli

Rust 製の汎用 Rhino プラグイン RPC クライアント + C# サーバライブラリ。RhinoCliPlugin は同梱のコア automation プラグインとして、AI エージェントが Rhino を起動・操作・診断するための標準 handler を提供する。

## プロジェクトステータス: Pre-alpha MVP

本プロジェクトはWIPであり、まだ設計を固めている段階。後方互換性より、CLI/API/プロトコルの責務がMECEであることを優先する。

- 設計上正しい変更を優先する。既存サブコマンド・handler 名・JSON schema を変更してよい
- deprecated 経路・互換シムは原則作らない。不要な分岐はAI自律操作の失敗原因になる
- ただし CLI/protocol/handler の変更は、同じ作業内でドキュメントを必ず追従する

## 最優先ルール: ドキュメント追従

ソースを変更したら、同じ作業の中で関連ドキュメントを更新する。別タスクに回さない。

- CLI サブコマンド・フラグ変更 → `README.md`, `docs/design.md`
- JSON-RPC protocol / builtin handler schema 変更 → `docs/protocol.md`, `docs/plugin-integration.md`
- RhinoCliPlugin handler 追加・変更 → `README.md`, `plugin/RhinoCliPlugin/README.md`, `docs/protocol.md`
- ビルド・テスト・配布手順変更 → `README.md`, `Makefile.toml`
- 実装タスクの進捗や検証履歴を残す場合 → `docs/tasks.md`

## 回答スタイル

- 挨拶・前置き・段階報告は不要。結論ファースト
- 指摘すべきことは率直に指摘する
- 不明点がある場合は必ず確認する

## コマンド

- Test: `cargo make test`
- Build: `cargo make build`
- Check: `cargo make check`
- Install CLI: `cargo make install-cli`
- Build plugin only: `cargo make build-plugin`
- Raw Rust tests: `cargo test -- --test-threads=1`
- Raw C# tests: `dotnet test server/RhinoCli.Server.Tests/RhinoCli.Server.Tests.csproj`

## Rhino 実機確認

- Plugin port 設定: `rhino-cli plugin set-port 50061`
- 起動: `rhino-cli launch --new-model`
- RPC待機: `rhino-cli wait-ready --port 50061 --timeout 120`
- 診断: `rhino-cli doctor --port 50061`
- Handler確認: `rhino-cli capabilities --format agent --port 50061`
- 終了: `rhino-cli shutdown`

`launch` はRhinoプロセス起動だけを担当する。port設定は `plugin set-port`、readiness待ちは `wait-ready`、実行中セッション内の新規モデル作成は `new-model` に分ける。この責務分離を崩さない。

## コミット

`/commit` スキルに従う。gitmoji形式（例: `✨ feat: ...`）。タイトル・本文は英語。

- NEVER: ユーザーの明示的な承認なしにコミット・プッシュしない
- NEVER: コミットメッセージに `Co-Authored-By` や `Generated with` 等のシグネチャを付けない

## ハンドラ追加の境界ポリシー

`rhino.*` namespace のハンドラを増やすときは、まず `rhino.run_python` で書けないか検討する。AddBox/AddSphere のような RhinoCommon ラッパーは作らない。

**ハンドラを追加してよい条件（全て満たすこと）:**

1. **run_python では実装困難**: RhinoCommon から触れない領域、UI スレッド以外の制御、特権操作、reflection で internal を叩く必要があるなど
2. **概念単位で1個に収束する**: 「Document を保存」のような閉じた概念で、形状・パラメータ次元で分裂しない
3. **構造化 I/O が本質的に意味を持つ**: 単に便利だからではなく、Python 側で文字列処理すると壊れやすい

**判定例:**

- `run_script` ✓: Rhino command system へのエントリ（Python 単独不可）
- `run_python` ✓: 自分自身がエスケープハッチ
- `probe_command` ✓: background thread で SendKeystrokes、ScriptContext 切替が必要
- `command_history` ✓: CommandHistoryViewModel を reflection で叩く
- `add_box` / `list_objects` / `delete_objects` / `save_document` / `capture_viewport` ✗: RhinoCommon 直叩きで完結。run_python + result_expression で代替

**run_python のレシピ集** は `docs/protocol.md` の `rhino.run_python` セクションに置く。よく使うパターン（save・open・add・list・delete・capture）は examples として 1 行で示す。

## 注意事項

- RhinoCliPlugin は `plugin/RhinoCliPlugin/RhinoCliPlugin.csproj` の PostBuild で Rhino の plugin directory にコピーされる。コピー先を直接編集しない
- 新しい handler は `HandlerMetadataAttribute` で metadata を付け、`capabilities` からAIが仕様を読める状態にする
- `RhinoCli.Server` は汎用サーバライブラリ。Rhino 固有操作を server built-in に入れず、同梱 plugin 側の handler として実装する
- `system.*` / `rpc.*` / `rhino.*` は予約 namespace。プラグイン固有 handler は `<plugin>.*` に分ける
- 実機機能の OS 別サポート:

  | 機能 | macOS | Windows native | WSL | pure Linux |
  |------|-------|----------------|-----|------------|
  | `launch` / `shutdown` / `app_running` | ✓ | ✓ | ✓ | ✗ |
  | `screenshot` | ✓ | ✗（別タスク） | ✗（別タスク） | ✗ |

  失敗時は `doctor`, `history`, `screenshot`（macOS のみ）を使って自律的に切り分ける。Windows / WSL では `RHINO_CLI_RHINO_EXE` で Rhino.exe を直接指せる

## 用語

- **RhinoCliPlugin**: rhino-cli 同梱のコア automation plugin。サンプルではなく標準 handler の実装場所
- **handler**: JSON-RPC method の実体。`rhino-cli call <method>` または専用CLIから呼ばれる
- **capabilities**: handler metadata を返す自己説明API。AIに使い方を伝える主要経路
