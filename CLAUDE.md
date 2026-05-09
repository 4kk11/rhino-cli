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

## 注意事項

- RhinoCliPlugin は `plugin/RhinoCliPlugin/RhinoCliPlugin.csproj` の PostBuild で Rhino の plugin directory にコピーされる。コピー先を直接編集しない
- 新しい handler は `HandlerMetadataAttribute` で metadata を付け、`capabilities` からAIが仕様を読める状態にする
- `RhinoCli.Server` は汎用サーバライブラリ。Rhino 固有操作を server built-in に入れず、同梱 plugin 側の handler として実装する
- `system.*` / `rpc.*` / `rhino.*` は予約 namespace。プラグイン固有 handler は `<plugin>.*` に分ける
- macOS 実機機能（launch/shutdown/screenshot）は権限・Rhino起動状態に依存する。失敗時は `doctor`, `history`, `screenshot` を使って自律的に切り分ける

## 用語

- **RhinoCliPlugin**: rhino-cli 同梱のコア automation plugin。サンプルではなく標準 handler の実装場所
- **handler**: JSON-RPC method の実体。`rhino-cli call <method>` または専用CLIから呼ばれる
- **capabilities**: handler metadata を返す自己説明API。AIに使い方を伝える主要経路
