# rhino-cli

Rust 製の汎用 Rhino プラグイン用 RPC クライアント + C# サーバライブラリ。任意の Rhino プラグインに JSON-RPC 2.0 over TCP インタフェースを追加し、CLI から呼び出して E2E テスト・自動化・スクリプティングを実現する。

> **Status**: Pre-alpha MVP. Rust CLI, C# server library, mock E2E runner, and MinimalPlugin example are implemented. Rhino manual E2E is still pending.

## 構成

| 場所 | 内容 |
|------|------|
| `src/` | Rust 製 CLI バイナリ (`rhino-cli`) |
| `server/RhinoCli.Server/` | C# クラスライブラリ (TCP server + Router + 組込 handler) |
| `examples/MinimalPlugin/` | 最小プラグイン例 (RhinoCli.Server を組み込む参考) |
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

## Quick Start

```bash
cargo build
cargo test
dotnet test server/RhinoCli.Server.Tests/RhinoCli.Server.Tests.csproj
dotnet build server/RhinoCli.TestRunner/RhinoCli.TestRunner.csproj
```

Install the CLI locally:

```bash
cargo install --path .
```

Run against a plugin server:

```bash
rhino-cli launch --port 50061 --timeout 120
rhino-cli wait-ready --port 50061 --timeout 30
rhino-cli ping --port 50061 --verbose
rhino-cli list-methods --port 50061
rhino-cli call system.version --port 50061 --pretty
rhino-cli shutdown
```

`launch` and `shutdown` currently automate Rhino on macOS via the installed app name. The default app is `Rhino 8`; use `--app "RhinoWIP"` or `--app "Rhino 7"` when needed. `launch --restart` asks Rhino to quit before relaunching. `launch --script "<Rhino command script>"` passes a Rhino `-runscript` argument before waiting for `system.ping`.

## MinimalPlugin

Build the Rhino example:

```bash
dotnet build examples/MinimalPlugin/MinimalPlugin.csproj
```

The build copies the plugin artifacts to:

```text
~/Library/Application Support/McNeel/Rhinoceros/8.0/MacPlugins/MinimalPlugin
```

Launch Rhino 8, then call:

```bash
rhino-cli launch --port 50099 --timeout 120
rhino-cli call minimal.hello --port 50099
rhino-cli shutdown
```

## Docs

- `docs/design.md` - architecture and scope
- `docs/protocol.md` - JSON-RPC 2.0 over TCP protocol
- `docs/plugin-integration.md` - embedding guide for existing plugins
- `docs/tasks.md` - implementation checklist

## ライセンス

MIT
