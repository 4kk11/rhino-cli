# rhino-cli 設計書

| 項目 | 内容 |
|------|------|
| プロジェクト | rhino-cli (汎用 Rhino プラグイン RPC クライアント + C# サーバライブラリ) |
| 設計日 | 2026-05-09 |
| ステータス | Draft |
| 前提読み | `docs/protocol.md` (プロトコル詳細) |

---

## 1. 概要

### 1.1 目的

Rhino プラグインは現状、対話的 GUI 経由でしか動作確認できない。本プロジェクトは任意の Rhino プラグインに JSON-RPC 2.0 over TCP インタフェースを後付けし、CLI からプログラマブルに呼び出せるようにする汎用基盤を提供する。

これにより以下が実現する:
- AI エージェントによる Rhino プラグインの自律 E2E テスト
- CI 不可能と思われていた回帰テストの実現
- 複数プラグイン横断のスクリプティング

### 1.2 直近の主要ユースケース

GeoMLRhino プロジェクトの **Stage 1.1 UserData 耐久テスト** を CLI 駆動で実行すること。50 通りの操作シナリオを Rhino API 経由でプログラマブルに走らせ、結果 JSON を CLI 経由で取得する。

長期的には GeoMLRhino 以外の任意のプラグイン (例: 既存の grasslang を rhino-cli に乗せ替える等) に展開可能な汎用基盤とする。

### 1.3 Goals

- プロトコルが **strict な JSON-RPC 2.0** であること (独自仕様にしない)
- C# サーバライブラリ側は **数行で各プラグインに組み込める** こと
- Rust CLI は **依存最小、単一バイナリ** であること
- 複数プラグインの **同時稼働** に耐える (それぞれ別ポート)

### 1.4 Non-Goals (MVP では明示的に除外)

- ストリーミング・通知 (server-sent notifications)
- 認証・暗号化 (loopback のみ動作)
- リモート (非ローカル) 接続
- 複数同時クライアントへのシリアライズ保証 (single-client 想定)
- Windows での開発検証 (動くべきだがテストは Mac のみ)
- バイナリ配布 (`cargo install --path .` のみ、後続)
- Rhino 7 / Rhino WIP サポート

---

## 2. アーキテクチャ

### 2.1 全体図

```
                         loopback TCP (default 50061)
                              JSON Lines (LF separator)
                              JSON-RPC 2.0 messages
┌────────────────────┐   ──────────────────────────►  ┌─────────────────────────────────┐
│   rhino-cli        │                                │  Rhino host process              │
│   (Rust binary)    │   ◄──────────────────────────  │   ├─ Plugin A (port 50061)       │
│                    │                                │   │   └─ RhinoCli.Server         │
│   subcommands:     │                                │   ├─ Plugin B (port 50062)       │
│    ping            │                                │   │   └─ RhinoCli.Server         │
│    list-methods    │                                │   └─ ...                         │
│    call            │                                │                                  │
│    wait-ready      │                                │  Each plugin runs its own server. │
└────────────────────┘                                └─────────────────────────────────┘
```

### 2.2 リポジトリ構成

```
rhino-cli/
├── README.md
├── LICENSE                       # MIT
├── .gitignore
├── Cargo.toml                    # CLI バイナリ
├── src/
│   ├── main.rs                   # clap エントリ
│   ├── client.rs                 # JSON-RPC client (TCP接続+送受信)
│   ├── protocol.rs               # request/response 型 (serde)
│   ├── error.rs                  # CLI 終了コード ↔ Result 変換
│   └── commands/
│       ├── mod.rs
│       ├── ping.rs
│       ├── list_methods.rs
│       ├── call.rs
│       └── wait_ready.rs
├── server/
│   └── RhinoCli.Server/
│       ├── RhinoCli.Server.csproj   # netstandard2.0 でも良いが net7.0 で揃える
│       ├── TcpServer.cs
│       ├── MessageRouter.cs
│       ├── HandlerRegistry.cs
│       ├── IHandler.cs
│       ├── RpcException.cs
│       └── Handlers/
│           ├── PingHandler.cs
│           ├── ListMethodsHandler.cs
│           └── ListPluginsHandler.cs
├── examples/
│   └── MinimalPlugin/            # RhinoCli.Server を組み込む最小例
│       ├── MinimalPlugin.csproj
│       ├── MinimalPlugin.cs
│       └── HelloHandler.cs
├── tests/
│   └── e2e_mock.rs               # CLI ↔ mock TCP server の Rust 統合テスト
└── docs/
    ├── design.md                 # 本書
    ├── tasks.md
    ├── protocol.md
    └── plugin-integration.md     # 既存プラグインへの組込手順 (Phase 6 で書く)
```

### 2.3 プロセス・スレッドモデル

**サーバ側 (C#)**:
- プラグイン `OnLoad` で `TcpServer.Start()` を呼ぶ
- `TcpListener` は accept loop をワーカースレッドで実行
- 1 接続 = 1 タスク (`Task.Run`) で `StreamReader.ReadLineAsync` ループ
- **handler 実行は必ず Rhino UI スレッド** (`RhinoApp.InvokeOnUiThread`)
- 同時接続は受け付けるが、handler 実行はシリアライズされる (UI スレッド経由のため自動的に)

**クライアント側 (Rust)**:
- `tokio` は使わない、同期 `std::net::TcpStream` で十分 (1 RPC = 1 接続 = 1 リクエスト)
- 接続 → JSON 1 行送信 → 1 行読む → 切断
- `wait-ready` のみ TCP 接続再試行ループを持つ

### 2.4 接続モデル

**connect-per-call** を MVP の唯一モードとする:
- CLI は呼び出しごとに新規 TCP 接続を張る
- サーバは接続クローズで状態リセット
- persistent / streaming は将来 (`--persistent` フラグや WebSocket 化など) 必要に応じて追加

理由: CLI 用途の 99% は単発呼び出し。永続接続は API 設計を複雑にし、エラー回復やバックプレッシャの考慮が必要になる。

---

## 3. プロトコル仕様 (要約)

詳細は `docs/protocol.md` を参照。要点のみ:

### 3.1 ベース

- **JSON-RPC 2.0** strict 準拠 (`"jsonrpc": "2.0"` 必須)
- **JSON Lines** で TCP 上に乗せる (1 メッセージ = 1 行 + LF)
- **UTF-8** エンコーディング、loopback (127.0.0.1) のみ bind

### 3.2 リクエスト

```json
{"jsonrpc": "2.0", "id": 1, "method": "system.ping", "params": null}
```

- `id`: number または string (null は notification、本実装では未対応)
- `method`: namespaced 推奨 (例: `system.ping`, `geoml.durability_test`)
- `params`: object または array、省略可 (= null)

### 3.3 成功レスポンス

```json
{"jsonrpc": "2.0", "id": 1, "result": {"pong": true}}
```

### 3.4 エラーレスポンス

```json
{"jsonrpc": "2.0", "id": 1, "error": {"code": -32601, "message": "Method not found", "data": null}}
```

### 3.5 標準エラーコード (JSON-RPC 2.0)

| Code | 意味 |
|------|------|
| -32700 | Parse error (JSON パース失敗) |
| -32600 | Invalid Request (構造不正) |
| -32601 | Method not found |
| -32602 | Invalid params |
| -32603 | Internal error |
| -32000 ～ -32099 | サーバ実装定義 (`RhinoCli.Server` で使う) |

### 3.6 メソッド命名規約

- 形式: `<namespace>.<method>` (snake_case)
- `system.*`: 接続・運用 (`ping`, `version`)
- `rpc.*`: イントロスペクション (`list_methods`, `list_plugins`)
- `<plugin>.*`: 各プラグイン固有 (例: `geoml.durability_test`)

namespace 衝突回避はプラグイン作者の責務。`system` / `rpc` は予約。

---

## 4. CLI 仕様

### 4.1 グローバルフラグ

| フラグ | 既定 | 説明 |
|--------|------|------|
| `--port <PORT>` | `50061` | 接続先ポート |
| `--host <HOST>` | `127.0.0.1` | 接続先ホスト (loopback 想定) |
| `--timeout <SEC>` | `60` | 応答タイムアウト |
| `--connect-timeout <SEC>` | `5` | 接続タイムアウト |
| `--pretty` | off | レスポンス JSON を整形出力 |
| `--raw` | off | `result` だけでなく レスポンス全体を出力 |
| `-q, --quiet` | off | エラー以外の stderr 出力を抑制 |
| `-v, --verbose` | off | デバッグ出力を有効化 |

環境変数で上書き可能:
- `RHINO_CLI_PORT`
- `RHINO_CLI_HOST`
- `RHINO_CLI_TIMEOUT`

### 4.2 サブコマンド

#### 4.2.1 `ping`

```
rhino-cli ping
```

`system.ping` を呼ぶ。応答は `{"pong": true, "server": "<plugin-id>"}` を期待。レイテンシを stderr に出す (`-v` で)。

#### 4.2.2 `list-methods`

```
rhino-cli list-methods
```

`rpc.list_methods` を呼び、結果を行で出力。

```
geoml.durability_test
rpc.list_methods
rpc.list_plugins
system.ping
system.version
```

#### 4.2.3 `call`

```
rhino-cli call <method> [PARAMS_JSON]
rhino-cli call <method> --params-file <PATH>
rhino-cli call <method> --param key=value [--param key=value ...]
```

汎用呼び出し。引数の優先順位:
1. `--params-file` (JSON ファイル全体を `params` に)
2. `--param key=value` の繰り返し (オブジェクトを構築)
3. 位置引数 1 個 (生 JSON 文字列)
4. 何もなし → `params: null`

成功時: `result` を stdout に JSON 出力 (デフォルト minified、`--pretty` で整形)。
失敗時: `error` を stderr に JSON 出力、終了コード 3。

#### 4.2.4 `wait-ready`

```
rhino-cli wait-ready [--timeout 30]
```

サーバ起動を待つ。指定秒間 100ms 間隔で `ping` を試行。`--timeout` の既定は `--connect-timeout` ではなく専用に 30 秒。Rhino 起動直後の race 用。

#### 4.2.5 `launch`

```
rhino-cli launch [--app "Rhino 8"] [--restart] [--no-wait] [--script "<RUNSCRIPT>"] [--timeout 120]
```

macOS 上で Rhino を起動し、対象ポートの `system.ping` が成功するまで待つ。既に応答可能な場合は即成功する。`--restart` は既存 Rhino に終了を依頼してから起動し直す。`--script` は Rhino の `-runscript` 引数として渡す。

#### 4.2.6 `shutdown`

```
rhino-cli shutdown [--app "Rhino 8"] [--timeout 30]
```

macOS の AppleScript 経由で Rhino に終了を依頼し、プロセス終了まで待つ。保存確認ダイアログが残る場合はタイムアウトする。

### 4.3 終了コード

| コード | 意味 |
|-------|------|
| 0 | 成功 |
| 1 | 一般エラー (引数解析失敗等) |
| 2 | 接続不可 (TCP connect 失敗、refused, timeout) |
| 3 | RPC エラー (応答に `error` フィールド) |
| 4 | 応答タイムアウト |
| 5 | 不正なレスポンス (JSON パース失敗、id 不一致) |

これにより Claude Code 側で `if [[ $? -ne 0 ]]` で原因をカテゴライズできる。

### 4.4 ヘルプ・バージョン出力

- `rhino-cli --help`: clap 標準
- `rhino-cli --version`: cargo パッケージバージョンを出力

---

## 5. C# サーバライブラリ API

`RhinoCli.Server` という名前空間で公開する。

### 5.1 公開クラス

#### 5.1.1 `TcpServer`

```csharp
public class TcpServer : IDisposable
{
    public TcpServer(int port, HandlerRegistry registry, string pluginId);
    public void Start();
    public void Stop();
    public int ActualPort { get; } // port=0 で auto 割当の場合用
    public event Action<string>? OnError; // 内部例外を上位で観測する用
}
```

- `pluginId`: `system.ping` の応答に含めるプラグイン識別子 (例: `"GeoMLRhino"`)
- accept loop と client loop は内部でワーカースレッド起動
- `Dispose()` は `Stop()` と同じ

#### 5.1.2 `HandlerRegistry`

```csharp
public class HandlerRegistry
{
    public HandlerRegistry();                      // 空 + system/rpc.* 自動登録
    public void Register(string method, IHandler handler);
    public bool Contains(string method);
    public IReadOnlyList<string> Methods { get; }
}
```

各プラグインはコンストラクタで registry を作り、`Register("geoml.durability_test", new DurabilityTestHandler(...))` で追加して `TcpServer` に渡す。

#### 5.1.3 `IHandler`

```csharp
public interface IHandler
{
    /// <summary>
    /// UI スレッド上で呼ばれる。同期戻り。
    /// </summary>
    /// <param name="params">JSON params (null 可)。</param>
    /// <returns>JSON シリアライズ可能な任意のオブジェクト。</returns>
    object? Execute(JsonNode? @params);
}
```

例外を投げると router が catch し、`-32603` Internal error または handler が投げた `RpcException` のコード/メッセージで応答する。

#### 5.1.4 `RpcException`

```csharp
public class RpcException : Exception
{
    public int Code { get; }
    public object? Data { get; }
    public RpcException(int code, string message, object? data = null);
}
```

`-32000 ～ -32099` の独自エラーや `-32602 Invalid params` を handler から明示的に出すために使う。

#### 5.1.5 `MessageRouter`

`TcpServer` の内部でのみ使う想定だが、テスト目的で公開:

```csharp
public class MessageRouter
{
    public MessageRouter(HandlerRegistry registry, string pluginId);
    public string HandleMessage(string jsonLine);   // 1 行 IN → 1 行 OUT
}
```

handler 実行は `MessageRouter` 自身は **呼ばない**。`TcpServer` 側で `RhinoApp.InvokeOnUiThread` を経由して呼ぶ。`MessageRouter` はパース・ルーティング・レスポンス整形のみに責務を絞る (テスト容易性のため)。

### 5.2 組込手順 (各プラグインから)

```csharp
// MyPlugin.cs
public class MyPlugin : Rhino.PlugIns.PlugIn
{
    public override PlugInLoadTime LoadTime => PlugInLoadTime.AtStartup;
    private TcpServer? _server;

    protected override LoadReturnCode OnLoad(ref string err)
    {
        var registry = new HandlerRegistry();   // system.* / rpc.* は自動登録済
        registry.Register("myplugin.do_thing", new DoThingHandler());
        _server = new TcpServer(port: 50063, registry, pluginId: "MyPlugin");
        _server.Start();
        return LoadReturnCode.Success;
    }

    protected override void OnShutdown() => _server?.Stop();
}
```

各プラグインは自分の既定ポートを README で宣言する責務を負う。

### 5.3 組込時の注意

- **ポート競合**: 複数プラグインが同じポートを取り合うと後ロードが失敗。プラグイン側でログを出して `LoadReturnCode.ErrorShowDialog` を返す
- **UI スレッド**: handler 内で `RhinoDoc` などを触る場合は既に UI スレッド上にいるので追加 invoke は不要
- **長時間処理**: handler 内でブロッキングすると Rhino UI が固まる。長時間処理は handler 内で別スレッドに投げ、進捗 ID を返して別の `*.status` メソッドでポーリングする設計を推奨

---

## 6. データフロー (1 RPC 呼び出し)

```
[CLI process]                                     [Rhino process / Plugin]

 main.rs::dispatch                                  TcpListener.AcceptLoop
   │                                                  │
   ▼                                                  ▼
 commands::call::run                                accept ─► spawn task
   │                                                  │
   ▼                                                  ▼
 client::Client::call(method, params)              StreamReader.ReadLineAsync
   │                                                  │
   ▼                                                  ▼
 build Request {jsonrpc, id, method, params}       MessageRouter.HandleMessage
   │                                                  │  ├─ JSON parse
   ▼                                                  │  ├─ method lookup
 TcpStream.connect(127.0.0.1:50061) ──────────────►│  └─ build invoke closure
   │                                                  │
   ▼                                                  ▼
 stream.write_all(json + \n) ─────────────────────►│  RhinoApp.InvokeOnUiThread
   │                                                  │  └─ handler.Execute(params)
   ▼                                                  ▼
 stream.read_line() ◄─────────────────────────────  StreamWriter.WriteLineAsync(json)
   │
   ▼
 parse Response, validate id matches request
   │
   ▼
 result?  → stdout JSON, exit 0
 error?   → stderr JSON, exit 3
```

---

## 7. 設定・環境変数

| 名前 | スコープ | 説明 |
|------|---------|------|
| `RHINO_CLI_PORT` | CLI | デフォルトポート上書き |
| `RHINO_CLI_HOST` | CLI | デフォルトホスト上書き |
| `RHINO_CLI_TIMEOUT` | CLI | デフォルトタイムアウト |
| `RHINO_CLI_DEBUG` | CLI | `1` で内部ログを stderr に出力 |

C# サーバ側はコンフィグファイルを持たない。プラグインの `OnLoad` で全部決める。

---

## 8. テスト戦略

### 8.1 Rust 側

- **ユニット**: `protocol.rs` のシリアライズ/パース、`client.rs` のエラー分類
- **統合**: `tests/e2e_mock.rs` で **モック TCP サーバ** を立てて、CLI 4 サブコマンド全てを fork-exec で呼ぶ
- 実 Rhino テストは Stage 1 ではスキップ (CI 不可)

### 8.2 C# 側

- **ユニット**: `MessageRouter.HandleMessage` の入力 → 出力対応 (UI スレッド要らない)
- **統合**: `RhinoCli.Server` 単独では Rhino 依存の handler は呼べないので、`PingHandler` / `ListMethodsHandler` のみテスト
- プラグイン組込テストは `examples/MinimalPlugin/` で目視確認

### 8.3 E2E (実 Rhino)

Stage 2 以降で考える。Rhino 起動が必要なため CI 化のハードルが高い。手動シナリオドキュメント (`docs/manual-e2e.md`) で代替する想定。

---

## 9. エラーモデル詳細

### 9.1 サーバ側のエラー → クライアント挙動

| サーバ動作 | クライアント観測 | CLI 終了コード |
|-----------|---------------|--------------|
| ポートで listen していない | TCP connect 失敗 (refused) | 2 |
| 接続中にプロセスが落ちる | TCP read で 0 バイト or RST | 5 |
| 不正な JSON を返す | パース失敗 | 5 |
| `id` が異なる | id 不一致 | 5 |
| `error.code = -32601` | RPC エラー | 3 |
| handler 内で例外 → `-32603` | RPC エラー | 3 |
| 応答が 60 秒以上来ない | タイムアウト | 4 |

### 9.2 クライアント側でのリトライ

`wait-ready` のみ自動リトライ。それ以外のサブコマンドはリトライしない (CLI ユーザの責務)。

---

## 10. 配布・インストール

### 10.1 MVP

- `git clone && cargo install --path .` のみ
- C# server は NuGet にも push しない。プラグイン側は project reference または `dotnet pack` 後 `*.nupkg` 直接参照

### 10.2 将来 (Phase 7 以降)

- `homebrew tap` 作成 → `brew install rhino-cli`
- C# は NuGet.org に push (`RhinoCli.Server`)
- GitHub Actions でリリースバイナリ自動生成 (mac-arm64, mac-x64, win-x64)

---

## 11. 既知の制約・将来課題

| # | 課題 | 解決時期 |
|---|------|---------|
| 1 | 複数同時クライアントの動作未検証 | E2E でニーズ出てから |
| 2 | 認証なし (loopback のみで安全と仮定) | リモート対応時 |
| 3 | UI スレッド占有時の長時間 RPC でフリーズ | 進捗ポーリングパターン推奨で逃げる |
| 4 | プラグイン間で同じ namespace を使うと衝突 | プラグイン作者の責務 |
| 5 | `--persistent` モードがない (毎回 connect) | 必要になったら追加 |
| 6 | C# 側は net7.0 のみ (net8.0 / netfx 検証なし) | Rhino 8 が net7.0 推奨のため当面これで |
| 7 | `system.version` の semver 仕様未確定 | プロトコル v0.1 凍結時に決定 |
| 8 | Rhino クラッシュ時の TCP 接続後始末 | OS まかせ |

---

## 12. 用語集

| 用語 | 意味 |
|------|------|
| handler | 1 つの RPC メソッドを処理する `IHandler` 実装 |
| registry | プラグイン内でメソッド名 → handler のマッピング |
| router | JSON 1 行を受け取り、registry を引いて応答 1 行を返すコンポーネント |
| pluginId | サーバの自己紹介文字列。`system.ping` 応答に含める |
| connect-per-call | CLI が 1 RPC ごとに TCP を張り直すモード |

---

## 13. 参考

- JSON-RPC 2.0 spec: <https://www.jsonrpc.org/specification>
- 参考実装: `.06_Projects/grasslang/` (TCP + JSON Lines + handler dictionary パターン)
- ホスト: <https://developer.rhino3d.com/guides/rhinocommon/>
