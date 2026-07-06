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
- バイナリ配布 (`cargo install --path .` のみ、後続)
- Rhino 7 / Rhino WIP サポート
- pure Linux (非 WSL) サポート (Rhino for Linux が存在しないため)
- `screenshot` の Windows / WSL 実装 (`PrintWindow`/`BitBlt` ベースの別タスク)

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
│    doctor          │                                │   └─ ...                         │
│    capabilities    │                                │                                  │
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
│       ├── list_plugins.rs
│       ├── call.rs
│       └── wait_ready.rs
├── server/
│   └── RhinoCli.Server/
│       ├── RhinoCli.Server.csproj   # netstandard2.0 でも良いが net8.0 で揃える
│       ├── TcpServer.cs
│       ├── MessageRouter.cs
│       ├── HandlerRegistry.cs
│       ├── IHandler.cs
│       ├── RpcException.cs
│       └── Handlers/
│           ├── PingHandler.cs
│           ├── ListMethodsHandler.cs
│           └── ListPluginsHandler.cs
├── plugin/
│   └── RhinoCliPlugin/           # rhino-cli 同梱の Rhino automation plugin
│       ├── RhinoCliPlugin.csproj
│       ├── src/RhinoCliPlugin.cs
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
- `rhino.*`: Rhino ホスト操作 (`run_script`, `command_history`)
- `<plugin>.*`: 各プラグイン固有 (例: `geoml.durability_test`)

namespace 衝突回避はプラグイン作者の責務。`system` / `rpc` / `rhino` は予約。

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

`rpc.list_methods` を呼び、結果を行で出力。互換用の軽量一覧で、AI や人間が handler の仕様を読む場合は `capabilities` を使う。

```
geoml.durability_test
rpc.capabilities
rpc.list_methods
rpc.list_plugins
system.ping
system.version
```

#### 4.2.3 `list-plugins`

```
rhino-cli list-plugins
```

`rpc.list_plugins` を呼び、`<plugin id>\t<port>` を 1 行ずつ出力する。MVP は要素 1 個固定だが、将来同一 Rhino プロセス内に複数の `RhinoCli.Server` が共存する場合に備えた発見 API。`--raw --pretty` で生 JSON を出す。

#### 4.2.4 `call`

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

#### 4.2.5 `wait-ready`

```
rhino-cli wait-ready [--timeout 30]
```

サーバ起動を待つ。指定秒間 100ms 間隔で `ping` を試行。`--timeout` の既定は `--connect-timeout` ではなく専用に 30 秒。Rhino 起動直後の race 用。ping 成功後に `rhino.run_python` で `Rhino.RhinoDoc.ActiveDoc` を 1 回だけ確認し、`None` のまま（典型的にはスタートウィンドウが残っていてプラグインのパネル / Python 経路が無音で失敗する状態）の場合は stderr に warning を出す（exit code は変えない）。`-q/--quiet` 指定時はこの追加チェックも抑制する。

#### 4.2.6 `doctor`

```
rhino-cli doctor [--app "Rhino 8"]
```

Rhino アプリの起動状態と `RhinoCliPlugin` RPC 到達性を確認する。未起動・ポート不一致・プラグイン未ロードの切り分けに使う。これは「使える状態か」を見る診断コマンドで、handler の仕様は `capabilities` に集約する。RPC が reachable な場合は追加で `rhino.run_python` 経由で `ActiveDoc` / `OpenDocuments` の状態を 1 行で報告し、`ActiveDoc` が `None` のまま（plugin パネル / Python 経路が無音で失敗する状態。典型的にはスタートウィンドウが残っているケース）を検知した場合は警告を出す。

#### 4.2.7 `capabilities`

```
rhino-cli capabilities [--method <METHOD>] [--format text|json|markdown|agent]
```

プラグイン側の `rpc.capabilities` を呼び、登録済み handler の説明、params/result schema、例、専用 CLI、side effects を表示する。AI に渡す文脈は `--format agent`、機械処理は `--format json` を使う。`--method` 指定時は 1 handler の詳細だけを出す。

#### 4.2.8 `launch`

```
rhino-cli launch [--app "Rhino 8"] [--restart] [--no-new-model] [--script "<RUNSCRIPT>"]
```

macOS / Windows native / WSL 上で Rhino を起動するだけのコマンド。port 概念は持たず、readiness 待ちもしない。RPC が応答するまで待つ必要がある場合は `rhino-cli wait-ready --port <PORT>` を別途呼ぶ。既に Rhino が起動済みの場合は no-op で成功する（起動時引数 `--script` を要求された場合は `--restart` 併用を促すエラーを返す）。`--restart` は OS ネイティブの quit 経路（macOS: AppleScript, Windows/WSL: `taskkill /IM Rhino.exe`）で既存 Rhino を終了させてから起動し直す。デフォルトでは起動時に harmless な `-runscript _NoEcho`（Windows では `/runscript=_NoEcho`）を渡して新規モデルウィンドウまで開き、`Rhino.RhinoDoc.ActiveDoc` を即時確定させる。スタートウィンドウ（最近使ったモデル / テンプレート選択）を残したい場合のみ `--no-new-model` を指定する。スタートウィンドウが残っている間は `ActiveDoc` が `None` のままになり、プラグインのパネル / Python 経路が無音で失敗するため、`wait-ready` と `doctor` がこの状態を検知して警告する。`--script` を指定するとデフォルトのスタートアップスクリプトを上書きし、Rhino の `-runscript` 引数として渡す。Windows/WSL では Rhino.exe を `RHINO_CLI_RHINO_EXE` → `C:\Program Files\Rhino {N}\System\Rhino.exe` の順で探索する（`--app "Rhino 7"` 等でバージョン優先度を変更可能）。pure Linux はサポート対象外。

bundled RhinoCliPlugin が listen するポートは `rhino-cli plugin set-port <PORT>` で事前に設定する（§4.2.9 参照）。サードパーティ plugin の port 設定はこの CLI のスコープ外で、各 plugin が自身の手段（env / 設定ファイル / ハードコード等）で解決する。

#### 4.2.9 `plugin set-port` / `plugin show-config`

```
rhino-cli plugin set-port <PORT>
rhino-cli plugin show-config
```

bundled RhinoCliPlugin の launch config (`~/Library/Application Support/rhino-cli/RhinoCliPlugin/config.json` on macOS) を書き換え／表示する。`set-port` は次回 Rhino ロード時に反映される。`show-config` は現在の内容と path を出力する（未生成なら fallback 仕様を案内）。サードパーティ plugin には影響しない。

#### 4.2.10 `shutdown`

```
rhino-cli shutdown [--app "Rhino 8"] [--timeout 30]
```

Rhino に終了を依頼し、プロセス終了まで待つ。OS ネイティブの quit 経路（macOS: AppleScript `quit app`, Windows/WSL: `taskkill /IM Rhino.exe` で WM_CLOSE 相当）を使うので、いずれも保存確認ダイアログを尊重する。ダイアログが残る場合はタイムアウトする。pure Linux はサポート対象外。

#### 4.2.11 `run-script`

```
rhino-cli run-script <SCRIPT> [--echo] [--mru <TEXT>] [--fail-on-false]
```

プラグイン側の `rhino.run_script` を呼び、Rhino UI スレッド上で `RhinoApp.RunScript` を実行する。`SCRIPT` は Rhino コマンドラインに渡す script 文字列。macOS の Rhino では script が command history に投入されても `RunScript` が `false` を返すケースがあるため、既定では結果 JSON を表示して終了コード 0 とする。厳密に false を失敗扱いしたい場合だけ `--fail-on-false` を使う。

#### 4.2.12 `history`

```
rhino-cli history [--tail <N>] [--json]
rhino-cli history --clear
```

プラグイン側の `rhino.command_history` / `rhino.clear_command_history` を呼ぶ。既定では command history のテキストだけを stdout に出す。`--json` または `--pretty` で line count や truncation 情報を含む JSON を出す。

#### 4.2.13 `new-model`

```
rhino-cli new-model [--template <3DM>]
```

プラグイン側の `rhino.new_model` を呼び、Rhino の default template から新規モデルを作成する。`--template` 指定時はその `.3dm` をテンプレートとして使う。これは既に modeling session が開いている状態で追加の新規モデルを作るための RPC で、起動画面を越える用途には `launch`（デフォルトで `_NoEcho` を渡す）を使う。

#### 4.2.14 `list-commands` / `probe-command`

```
rhino-cli list-commands [--pattern <P>] [--include-unloaded]
rhino-cli probe-command <NAME>
```

`list-commands` は `rhino.list_commands` を呼び、Rhino に登録済みのコマンド名を返す（既定は英語名・ロード済みのみ、`--pattern` で case-insensitive substring filter、`--include-unloaded` で未ロード分も含める）。`probe-command` は `rhino.probe_command` を呼び、コマンドを `! _-{Name} _Cancel` で起動して即時中断し、その間に Rhino が `RhinoApp.CommandPrompt` に置いた最初のプロンプト（option labels 込み）を捕獲して返す。docs URL に依存せず動的に option を発見できる。option short code（`(D)` 等の括弧内 1〜2 文字）は ASCII 安定で `_D` 等としてそのまま `run-script` に渡せるため、prompt 本文が locale 出力でも実用上問題ない。`-` プレフィックスを持たないコマンドや起動と同時に side effect を起こすコマンドでは挙動が崩れる点に注意。

AI agent の使い分け:
- まず `list-commands` で候補を絞る
- `probe-command` で実機の最初のプロンプト・オプションを動的取得する
- 取得した option short code と座標 syntax (`x,y,z`) で `run-script` を組み立てて実行する

#### 4.2.15 `inspect-type`

```
rhino-cli inspect-type <FQN> [--binding <B>] [--include-inherited]
```

`rhino.inspect_type` を呼び、Rhino プロセスにロード済みの .NET 型を
`System.Reflection` で内省して JSON で返す。出力は constructors / properties /
methods（オーバーロードはグルーピング）/ events / fields の構造化情報。
`run_python` で RhinoCommon を直接叩く前に、AI が constructor の引数型や
property の static 性を確認するための **API 発見ハンドラ**。

型解決は **FQN のみ**（`Rhino.Geometry.Box` 等）。末尾一致フォールバックは
誤マッチ防止のため採用しない。短い名前から FQN を引きたい場合は将来追加する
`search-types` を使う前提。`--binding` は `public` / `public_instance` /
`public_static` / `non_public` / `all` から選択（既定 `public` =
Public Instance + Public Static）。`--include-inherited` で親クラスのメンバも
含める（既定 DeclaredOnly）。

XML doc `<summary>` 取り込みは Phase B、メソッド body の C# 復元は Phase D
で `decompile-method` として別ハンドラに追加予定。

#### 4.2.16 `search-types`

```
rhino-cli search-types <PATTERN> [--scope all|types|members] [--assembly <NAME>] [--limit <N>]
```

`rhino.search_types` を呼び、ロード済みアセンブリから型名 / メンバ名の
部分一致（case-insensitive）を返す。`inspect-type` が **FQN のみ**を
受理する前提なので、AI が短い名前しか知らないときの **FQN 解決ステップ**
として使う。

デフォルト assembly フィルタは `Rhino*` / `RhinoCommon` / `RhinoCli*` の
prefix。広げたい場合は `--assembly <NAME>` で完全一致指定。出力は
`limit` (既定 50) で打ち切られ、超過時は `truncated: true`。

`type.IsVisible` で internal 型は除外し、property/event accessor などの
`IsSpecialName` メンバも除外する。

典型ワークフロー: `search-types AddBox` → `Rhino.DocObjects.Tables.ObjectTable.AddBox`
を発見 → `inspect-type Rhino.DocObjects.Tables.ObjectTable` で overload を確認 →
`run-python` で実装。

#### 4.2.17 `decompile-method`

```
rhino-cli decompile-method <TYPE_FQN> <METHOD> [--signature <SIG>]
```

`rhino.decompile_method` を呼び、ICSharpCode.Decompiler でメソッド本体の
IL を C# 復元して返す。`inspect-type` がシグネチャ（インターフェース）
だけを返すのに対し、これは **実装** を返す。AI がメソッドの内部処理を
読みたい場面（edge case 推測、複雑な API のデバッグ）に使う。

コンストラクタを decompile するときは `METHOD` に `.ctor` を指定。
オーバーロードがあるときは `--signature` でカンマ区切りの型名で絞り込む。
型名は FullName (`Rhino.Geometry.Point3d`) でも 短縮形 (`Point3d`) でも
受理する。曖昧な指定は `-32602 / ambiguous_overload` で候補シグネチャの
配列を返す。

Decompiler は assembly path 単位でキャッシュされ、初回呼び出し時のみ
RhinoCommon 全体の型システム構築のため数百 MB を確保する。プロセス停止
までキャッシュは維持される。

`inspect-type --with-body <METHOD>` は CLI 側のオプション合成で、
`inspect_type` の結果に対し指定メソッドの全 overload について
`decompile_method` を呼び、`methods[*].overloads[*].body` に C# を merge
する。ハンドラ自体は分離維持。

#### 4.2.18 `screenshot`

```
rhino-cli screenshot [--app "Rhino 8"] [--out <PNG>] [--window-id <ID>] [--no-activate] [--no-shadow]
```

**macOS のみ対応**（Windows / WSL は `PrintWindow`/`BitBlt` ベースの別タスクで実装予定）。Rhino アプリの前面ウィンドウを PNG に保存する。RPC サーバやプラグイン handler には依存しないため、Rhino が起動していれば `run-script` や `history` の結果と合わせて AI エージェントが視覚的にデバッグできる。`--out` 未指定時は `rhino-screenshot-<unix>.png` をカレントディレクトリに作る。`--no-shadow` は macOS のウィンドウ影を除外し、`--window-id` は既知の window id を直接指定する。実行端末には macOS の Screen Recording 権限が必要。Windows / WSL ターゲットでは明示エラーを返す。

#### 4.2.19 `execute-panel-js`

```
rhino-cli execute-panel-js <PANEL_GUID> <SCRIPT>
```

`rhino.execute_in_panel_webview` の専用 CLI。GUID で `Rhino.UI.Panels.GetPanel` を引き、その Eto control 木から最初の `Eto.Forms.WebView` を見つけて JS を実行する。`<SCRIPT>` は IIFE wrap されるので `return <expr>` で値を返せる。戻り値は handler 側で `JSON.stringify` してから JSON.parse され、`{ status, value, panel_type }` で返る。AICmdHub / Lattice のような WebView panel plugin を、private field の reflection ハックなしで自律デバッグするための専用経路。詳細は `docs/protocol.md` §3.6.5。

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

handler の説明、params/result schema、例、side effects は handler クラスの `HandlerMetadataAttribute` に持たせる。`HandlerRegistry.Register(method, handler)` はこの attribute を自動で読み、`rpc.capabilities` で公開する。登録時に明示 `HandlerMetadata` を渡した場合はそちらを優先する。

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
- プラグイン組込テストは `plugin/RhinoCliPlugin/` で目視確認

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
| 6 | C# 側は net8.0 のみ (netfx 検証なし) | Rhino 8 SR で net8.0 サポート済み |
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
