# rhino-cli

Rust 製の汎用 Rhino プラグイン用 RPC クライアント + C# サーバライブラリ。任意の Rhino プラグインに JSON-RPC 2.0 over TCP インタフェースを追加し、CLI から呼び出して E2E テスト・自動化・スクリプティングを実現する。

> **Status**: Pre-alpha (設計フェーズ)。実装はまだ存在しない。`docs/design.md` と `docs/tasks.md` を参照。

## 構成

| 場所 | 内容 |
|------|------|
| `src/` | Rust 製 CLI バイナリ (`rhino-cli`) |
| `server/RhinoCli.Server/` | C# クラスライブラリ (TCP server + Router + 組込 handler) |
| `examples/minimal-plugin/` | 最小プラグイン例 (RhinoCli.Server を組み込む参考) |
| `docs/design.md` | 設計書 |
| `docs/tasks.md` | 実装タスクリスト |
| `docs/protocol.md` | JSON-RPC 2.0 プロトコル詳細 |

## クイック概念

```
┌──────────────┐    JSON-RPC 2.0 over TCP   ┌────────────────────────────────┐
│  rhino-cli   │  ────────────────────────► │ Rhino plugin                   │
│  (Rust bin)  │  ◄──────────────────────── │  └ RhinoCli.Server (C# lib)    │
│              │                             │     ├ TcpServer                │
│              │                             │     ├ MessageRouter            │
│              │                             │     ├ built-in handlers        │
│              │                             │     └ plugin-specific handlers │
└──────────────┘                             └────────────────────────────────┘
```

各 Rhino プラグインは `RhinoCli.Server` を NuGet 依存として組み込み、自身の handler だけ登録する。`rhino-cli` は (現状の Rhino で動いている) いずれかのプラグインのポートに接続して RPC を発行する。

## 想定ユースケース

- **E2E テスト**: 複数プラグインの自動回帰テスト (Claude Code が自律実行)
- **自動化**: バッチジョブ・CI 連携
- **デバッグ**: live な Rhino インスタンスへの状態クエリ

## 依存対象

- Rust 1.75+ (CLI ビルド)
- .NET 7.0 SDK (server lib ビルド)
- Rhino 8 (host)

## ライセンス

MIT
