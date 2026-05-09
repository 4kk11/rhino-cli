# rhino-cli 実装タスクリスト

| 項目 | 内容 |
|------|------|
| 対象 | rhino-cli (Rust CLI) + RhinoCli.Server (C# library) |
| 設計書 | `docs/design.md` |
| プロトコル | `docs/protocol.md` |
| 作成日 | 2026-05-09 |
| テスト方針 | TDD は protocol/router など純ロジックに限定。TCP I/O や Rhino UI スレッド依存箇所は実装後の統合テスト |
| 進捗管理 | チェックボックス `- [ ]` を `- [x]` に更新、Phase ヘッダに ✅ 完了 を追記 |

---

## Phase 0: スキャフォールド ✅ 完了 (本ドキュメント生成と同時)

- [x] **0-1**: ディレクトリ作成 (`src/commands`, `server/RhinoCli.Server/Handlers`, `examples`, `tests`, `docs`)
- [x] **0-2**: `README.md` 作成
- [x] **0-3**: `LICENSE` 作成 (MIT)
- [x] **0-4**: `.gitignore` 作成
- [x] **0-5**: `docs/design.md` 作成
- [x] **0-6**: `docs/protocol.md` 作成
- [x] **0-7**: `docs/tasks.md` 作成 (本ファイル)

---

## Phase 1: Rust プロトコル層 (純ロジック) ✅ 完了

JSON-RPC 2.0 のシリアライズ/パースだけを担当。TCP は触らない。

### 🔴 Red

- [x] **1-1**: `Cargo.toml` を作成 (deps: `serde`, `serde_json`, `clap` (derive))
- [x] **1-2**: `tests/protocol_test.rs` でリクエスト構築・レスポンスパースのテストを書く
  - 正常リクエスト → JSON 文字列
  - 正常レスポンス JSON → 構造体
  - エラーレスポンス JSON → 構造体
  - 不正 JSON → パースエラー
  - `result` と `error` 両方ある不正レスポンス → エラー扱い
  - `id` 不一致を検出する関数の挙動

### 🟢 Green

- [x] **1-3**: `src/protocol.rs` を作成
  - `Request { jsonrpc, id, method, params }`
  - `Response { jsonrpc, id, result, error }`
  - `RpcError { code, message, data }`
  - `Id` enum (Number / String)
  - `to_json_line()`, `from_json_line()` ヘルパ
- [x] **1-4**: `src/error.rs` を作成
  - `CliError` 列挙 (Connect, Timeout, RpcError, Parse, Other)
  - `From<io::Error>`, `From<serde_json::Error>` 実装
  - `exit_code()` メソッド (設計書 4.3 のマッピング)

### 🔵 Refactor

- [x] **1-5**: doc コメント追加、テスト網羅 (空 params, 大きな数値 ID 等)

---

## Phase 2: Rust クライアント層 ✅ 完了

TCP 接続を持つ。実 Rhino には繋がない (Phase 5 で mock サーバを立ててテスト)。

### 🔴 Red

- [x] **2-1**: `tests/client_test.rs` で mock TCP listener を立て、`Client::call` の挙動を検証する骨組み
  - 正常系: 1 行 IN → 1 行 OUT
  - サーバ側で connection refused → `CliError::Connect`
  - サーバが応答せずタイムアウト → `CliError::Timeout`
  - サーバが切断 → `CliError::Parse`
  - id ミスマッチ → エラー

### 🟢 Green

- [x] **2-2**: `src/client.rs` を作成
  - `Client { host, port, connect_timeout, read_timeout }`
  - `Client::call(method: &str, params: Value) -> Result<Value, CliError>`
  - 中身: TcpStream::connect_timeout → write_all → BufRead.read_line → 切断
  - id は呼び出しごとに `AtomicU64` で採番

### 🔵 Refactor

- [x] **2-3**: 設定値の env var フォールバック (`RHINO_CLI_*`)
- [x] **2-4**: `--verbose` 用に `tracing` か `eprintln!` で I/O ログ

---

## Phase 3: Rust CLI サブコマンド ✅ 完了

clap でサブコマンドを定義し、Phase 2 の Client を呼び出す。

### 🔴 Red

- [x] **3-1**: `tests/cli_test.rs` で `assert_cmd` を使った integration test 骨組み (mock サーバで)

### 🟢 Green

- [x] **3-2**: `src/main.rs` で clap derive を使った CLI 定義
- [x] **3-3**: `src/commands/ping.rs`
  - `system.ping` を呼び `pong` を確認、stderr に `pong from <server> <ver> (<latency>ms)` (verbose 時)
- [x] **3-4**: `src/commands/list_methods.rs`
  - `rpc.list_methods` を呼び結果配列を改行で出力
- [x] **3-5**: `src/commands/call.rs`
  - 引数優先順位: `--params-file` > `--param key=value (複数)` > 位置引数 > null
  - 成功 `result` を stdout、`--pretty` で整形
- [x] **3-6**: `src/commands/wait_ready.rs`
  - 既定 30 秒、100 ms 間隔で ping、最初の成功で 0 を返す
- [x] **3-7**: 終了コードを `CliError::exit_code()` で統一して `process::exit`

### 🔵 Refactor

- [x] **3-8**: ヘルプ文の日本語/英語混在チェック → 英語に統一
- [x] **3-9**: `--quiet` `--verbose` の挙動を共通化

---

## Phase 4: C# サーバライブラリ (RhinoCli.Server) ✅ 完了

Rhino 非依存の純粋ライブラリ。`MessageRouter` / `HandlerRegistry` / `IHandler` のみここで完結。`TcpServer` は Rhino UI スレッド依存があるが thin wrapper にする。

### 🔴 Red

- [x] **4-1**: `server/RhinoCli.Server.Tests/` プロジェクト作成 (xUnit)
- [x] **4-2**: `MessageRouterTests.cs`
  - 正常: ping → 成功応答
  - method not found → -32601
  - parse error → -32700
  - invalid request (no method) → -32600
  - handler が `RpcException` を投げる → そのコード/メッセージで応答
  - handler が generic 例外 → -32603

### 🟢 Green

- [x] **4-3**: `server/RhinoCli.Server/RhinoCli.Server.csproj`
  - net7.0、`System.Text.Json` 使用 (Newtonsoft 依存しない)
- [x] **4-4**: `IHandler.cs`
- [x] **4-5**: `RpcException.cs`
- [x] **4-6**: `HandlerRegistry.cs` (system/rpc.* の auto-register 含む)
- [x] **4-7**: `MessageRouter.cs`
  - JSON line in/out
  - jsonrpc=="2.0" 検証
  - id 抽出 (number / string / 欠落)
  - errorResponse / successResponse 整形
  - `IHandler.Execute` を 呼ぶ部分は **delegate を取って外部に委譲** (router 自体は UI スレッドを知らない)
- [x] **4-8**: `TcpServer.cs`
  - `TcpListener` ループ、`StreamReader.ReadLineAsync` クライアントループ
  - `MessageRouter.HandleMessage` を呼ぶ delegate に `Action<IHandler, JsonNode?>` を渡す
  - そこで `RhinoApp.InvokeOnUiThread` を呼ぶ (Rhino 依存はここだけ)
  - `Start()` `Stop()` `Dispose()`

### 🔵 Refactor

- [x] **4-9**: スレッドセーフティ確認 (handler registry は読み取り専用、TcpServer の cts 周り)
- [x] **4-10**: 接続エラーログを `OnError` event で外に出す

---

## Phase 5: 組込 handler (system / rpc.*) ✅ 完了

### 🔴 Red

- [x] **5-1**: `MessageRouterTests.cs` に system.ping, system.version, rpc.list_methods, rpc.list_plugins のテストケースを追加

### 🟢 Green

- [x] **5-2**: `Handlers/PingHandler.cs`
- [x] **5-3**: `Handlers/VersionHandler.cs` (`system.version`)
- [x] **5-4**: `Handlers/ListMethodsHandler.cs`
- [x] **5-5**: `Handlers/ListPluginsHandler.cs` (MVP は固定 1 件)
- [x] **5-6**: `HandlerRegistry` ctor で上記 4 件を auto-register

### 🔵 Refactor

- [x] **5-7**: `pluginId` / `version` 文字列が registry → handler に正しく流れているか確認

---

## Phase 6: Rust ↔ C# E2E (mock-less mini) ✅ 完了

実際に Rust CLI が C# サーバと話せることを検証する。Rhino には依存しない。

### 🔴 Red

- [x] **6-1**: `tests/e2e_mock.rs`
  - `plugin/RhinoCliPlugin/` を Rhino なしで動かすことは難しいので、代わりに **C# console runner** (`server/RhinoCli.TestRunner/`) を作って、`InvokeOnUiThread` の代わりに `Action.Invoke()` を直接呼ぶダミー実装で TCP server だけ立てる
  - Rust 側は `Command::new("dotnet").args(...)` で起動し、port を待ち、call を試す
  - シナリオ: ping、list-methods、不明メソッド (-32601)、parse error (`echo` で不正 JSON 投げて -32700 確認)

### 🟢 Green

- [x] **6-2**: `server/RhinoCli.TestRunner/RhinoCli.TestRunner.csproj` 作成 (net7.0 console app)
- [x] **6-3**: `Program.cs` で `TcpServer` を立てて `Console.ReadKey` で待機 (テストから kill)
- [x] **6-4**: Rust テスト側で起動・接続・終了の orchestration

### 🔵 Refactor

- [x] **6-5**: ポート競合回避 (port=0 で auto 割当 → ActualPort を stdout に出力)

---

## Phase 7: RhinoCliPlugin ✅ 完了

実際の Rhino プラグインから RhinoCli.Server を組み込む参考例。**ビルドのみ確認**、実起動は手動。

### 🟢 Green

- [x] **7-1**: `plugin/RhinoCliPlugin/RhinoCliPlugin.csproj`
  - net7.0、RhinoCommon 依存、`RhinoCli.Server` を ProjectReference
- [x] **7-2**: `RhinoCliPlugin.cs`
  - `OnLoad` で `TcpServer` 起動 (default port 50061 / launch config で上書き)
  - 1 つだけ診断 handler `rhino_cli.echo` (params をそのまま result に)
- [x] **7-3**: `RhinoCliPlugin/HelloHandler.cs` (`rhino_cli.hello` → `{"hello":"world"}`)
- [x] **7-4**: `plugin/RhinoCliPlugin/README.md` で起動・接続手順
- [x] **7-5**: Rhino を実際に立ち上げて手動確認:
  - `rhino-cli ping --port 50061`
  - `rhino-cli call rhino_cli.hello --port 50061`

---

## Phase 8: ドキュメント仕上げ + 配布準備

- [x] **8-1**: `docs/plugin-integration.md` を書く (既存プラグインへの組込手順)
- [x] **8-2**: `README.md` のクイックスタートを最終形に更新
- [ ] **8-3**: GitHub リポジトリ作成 (publicまたはprivate、未確定)
- [x] **8-4**: `cargo install --path .` での動作確認
- [ ] **8-5**: バージョン `0.1.0` をタグ付け

---

## 実装順序 (依存グラフ)

```
Phase 0 ✅
   │
   ▼
Phase 1 (Rust 純ロジック)
   │
   ├─────────────► Phase 4 (C# router 純ロジック)
   │                   │
   ▼                   ▼
Phase 2 (Rust client)  Phase 5 (C# 組込 handler)
   │                   │
   ▼                   ▼
Phase 3 (CLI サブコマンド)
   │
   ▼
Phase 6 (Rust ↔ C# E2E、Rhino なし)
   │
   ▼
Phase 7 (RhinoCliPlugin、Rhino あり 手動)
   │
   ▼
Phase 8 (docs + 配布)
```

Phase 1 と Phase 4 は **並行可能** (純ロジックで相互に依存しない)。
Phase 2 と Phase 5 は **並行可能** (TCP 層は別、router 内部完結)。
Phase 6 で初めて両者が出会う。

---

## Phase 完了の定義 (Definition of Done)

各 Phase は以下を満たした時点で完了:
1. すべてのチェックボックスが `[x]`
2. テストが通る (`cargo test` / `dotnet test`)
3. 該当 phase のヘッダに `✅ 完了` を追記

---

## 本リポジトリのスコープ外 (= GeoMLRhino 側で別タスク)

以下はこのリポジトリでは扱わない:

- GeoMLRhino プラグイン本体への `RhinoCli.Server` 組込
- `geoml.durability_test` handler の実装
- Stage 1.1 シナリオの C# コード化
- 実 Rhino を使った E2E テスト
- GitHub Actions CI

これらは GeoMLRhino リポジトリのタスクとして別途追跡する。

---

## 進捗ログ

| Date | Phase | Note |
|------|-------|------|
| 2026-05-09 | 0 | 設計書・プロトコル仕様・タスクリスト作成完了 |
| 2026-05-09 | 1-3 | Rust protocol/client/CLI 実装、mock TCP integration test 通過 |
| 2026-05-09 | 4-6 | C# server library、built-in handlers、Rust↔C# E2E runner 実装 |
| 2026-05-09 | 7-8 | RhinoCliPlugin build、plugin integration docs、cargo install 確認 |
| 2026-05-09 | extra | RhinoCliPlugin PostBuild コピー、Rhino launch/shutdown CLI 追加 |
| 2026-05-09 | 7 | `rhino-cli launch --port 50061` → `call rhino_cli.hello` → `shutdown` 実機確認 |
| 2026-05-09 | extra | Rhino run-script/history CLI と RhinoCliPlugin handlers 追加 |
| 2026-05-09 | extra | Rhino window screenshot CLI 追加 |
| 2026-05-09 | verify | `screenshot` 実機実行で macOS Screen Recording 権限チェックまで確認 |
| 2026-05-09 | extra | `launch --new-model` と `rhino.new_model` handler 追加 |
| 2026-05-09 | extra | `examples/MinimalPlugin` を `plugin/RhinoCliPlugin` へ移動し、コア同梱プラグインとして改名 |
| 2026-05-09 | extra | `doctor` / `capabilities` と `rpc.capabilities` metadata を追加 |
| 2026-05-09 | extra | handler metadata を `HandlerMetadataAttribute` に移し、handler クラス定義へ集約 |
