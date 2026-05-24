# rhino-cli

Rhino プラグイン向けの汎用 JSON-RPC 2.0 クライアント (Rust 製 CLI) と C# サーバライブラリ。任意の Rhino プラグインにサーバライブラリを組み込むことで、操作を TCP 越しに公開し、CLI から E2E テスト・自動化・スクリプティングを駆動できる。同梱の `RhinoCliPlugin` は AI エージェントや人間が共通して使う標準 Rhino 自動化ハンドラを提供する。

> ステータス: Pre-alpha MVP。Rust CLI、C# サーバライブラリ、モック E2E ランナー、`RhinoCliPlugin` のいずれも実装済み。

English: [README.md](README.md)

## アーキテクチャ

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

各 Rhino プラグインは `RhinoCli.Server` を NuGet 依存として組み込み、自身のハンドラを登録する。`rhino-cli` は現在 Rhino プロセス内で稼働している任意のプラグインのポートに接続して RPC を発行する。

## リポジトリ構成

| パス | 内容 |
|------|------|
| `src/` | Rust 製 CLI バイナリ (`rhino-cli`) |
| `server/RhinoCli.Server/` | C# クラスライブラリ: TCP サーバ + メッセージルータ + 組込ハンドラ |
| `server/RhinoCli.Server.Tests/` | サーバユニットテスト |
| `server/RhinoCli.TestRunner/` | モック E2E ランナー |
| `plugin/RhinoCliPlugin/` | 同梱 Rhino automation プラグイン |
| `docs/` | 設計・プロトコル・統合・タスクリスト |

## ユースケース

- 複数の Rhino プラグインを横断する E2E 回帰テスト (AI エージェントが自律実行)
- バッチ自動化・CI 連携
- 稼働中の Rhino インスタンスへの live なステート照会とデバッグ

## 要件

- Rust 1.75+ (CLI ビルド用)
- .NET 7.0 SDK (サーバライブラリ・プラグインビルド用)
- Rhino 8 (ホストアプリケーション)

## インストール

CLI は [crates.io](https://crates.io/crates/rhino-cli) に、同梱の Rhino プラグインは [Yak サーバ](https://yak.rhino3d.com) に公開する。両方を入れるには:

```bash
# CLI
cargo install rhino-cli

# RhinoCliPlugin (Rhino 8 のコマンドラインから、または Package Manager UI を使う)
_PackageManager
# Search for: rhino-cli
```

Yak CLI で直接インストールする場合:

```bash
"/Applications/Rhino 8.app/Contents/Resources/bin/yak" install rhino-cli
```

## ソースからビルド

推奨タスクランナー (cargo-make):

```bash
cargo install cargo-make
cargo make check
cargo make install-cli
```

個別タスク:

```bash
cargo make test
cargo make build
cargo make install-cli
cargo make build-plugin
```

同等の raw コマンド:

```bash
cargo build
cargo test -- --test-threads=1
dotnet test server/RhinoCli.Server.Tests/RhinoCli.Server.Tests.csproj
dotnet build server/RhinoCli.TestRunner/RhinoCli.TestRunner.csproj
cargo install --path .
```

## プラグインサーバへの接続

同梱プラグインに対する典型的なセッション:

```bash
rhino-cli plugin set-port 50061
rhino-cli launch
rhino-cli wait-ready --port 50061 --timeout 120
rhino-cli doctor --port 50061
rhino-cli capabilities --format agent --port 50061
rhino-cli call system.version --port 50061 --pretty
rhino-cli new-model --port 50061
rhino-cli run-script "_Zoom _Extents" --port 50061
rhino-cli capture-viewport --width 1280 --height 720 --out /tmp/viewport.png --port 50061
rhino-cli shutdown
```

責務は意図的に分離されている: `launch` は Rhino プロセスを起動するだけ、`plugin set-port` は同梱プラグインの listening ポートを設定するだけ、`wait-ready` は RPC エンドポイントが `system.ping` に応答するまでブロックするだけ。これらを組み合わせて使うこと。単一コマンドに 3 つの役割を兼務させない。デフォルトでは `launch` は起動時に `-runscript _NoEcho` を渡して新規モデリングウィンドウを開き、`RhinoDoc.ActiveDoc` が即座に確定するようにする。スタートウィンドウ (最近使ったモデル / テンプレート選択) を残したい場合のみ `--no-new-model` を指定する (この場合は手動でスタートウィンドウを閉じるまで `ActiveDoc` が `None` のままになり、プラグインのパネル / Python 操作は無音で失敗する)。`wait-ready` と `doctor` は plugin が reachable になった後に `ActiveDoc` が `None` のままなら警告を出す。

## CLI サブコマンドリファレンス

| コマンド | 主なフラグ | 説明 |
|---------|-----------|------|
| `ping` | — | `system.ping` を呼ぶ。 |
| `doctor` | `--app` | Rhino + プラグインの到達性を診断する。 |
| `capabilities` | `--method`, `--format {text,json,markdown,agent}` | ハンドラの自己説明カタログを表示する。 |
| `call` | `<method> [params_json]`, `--params-file`, `--param k=v` | 任意ハンドラへの汎用エントリポイント。 |
| `list-methods` | — | 登録済み RPC メソッド名を列挙する。 |
| `list-plugins` | — | 到達可能な `RhinoCli.Server` プラグインインスタンスを列挙する (`<id>\t<port>`)。 |
| `wait-ready` | (`--timeout` を使用) | `system.ping` が成功するまでブロックする。 |
| `launch` | `--app`, `--restart`, `--no-new-model`, `--script` | Rhino を起動する (macOS)。デフォルトで新規モデルを開き `ActiveDoc` を確定させる。 |
| `shutdown` | `--app` | Rhino に終了を依頼し、終了を待つ (macOS)。 |
| `plugin set-port` | `<port>` | 同梱 `RhinoCliPlugin` の起動コンフィグを書き込む。 |
| `plugin show-config` | — | 現在の同梱プラグイン起動コンフィグを表示する。 |
| `screenshot` | `--out`, `--no-shadow`, `--no-activate`, `--window-id` | `screencapture` 経由で macOS ウィンドウを撮影する。 |
| `capture-viewport` | `--width`, `--height` (必須), `--viewport`, `--mode`, `--projection`, `--camera`, `--target`, `--zoom-extents`, `--transparent`, `--out` | プラグイン経由で 1 ビューポートを撮影する (PNG + base64)。 |
| `new-model` | `--template <3dm>` | 新しいアクティブドキュメントを作成する。 |
| `run-script` | `<script>`, `--echo`, `--mru`, `--fail-on-false` | Rhino コマンドスクリプトを実行する。 |
| `history` | `--tail`, `--clear`, `--json` | Rhino コマンド履歴を読む / クリアする。 |
| `list-commands` | `--pattern`, `--include-unloaded` | Rhino コマンド名を列挙する。 |
| `probe-command` | `<name>` | コマンドを起動し、最初のプロンプトとオプションラベルを取得する。 |
| `inspect-type` | `<fqn>`, `--binding`, `--include-inherited`, `--with-body` | Rhino プロセス内にロードされた .NET 型をリフレクションする。 |
| `search-types` | `<pattern>`, `--scope`, `--assembly`, `--limit` | 型・メンバーを部分一致 (大小無視) で検索する。 |
| `decompile-method` | `<type> <method>`, `--signature` | 1 メソッドを C# に逆コンパイルする。 |

グローバルフラグ (全サブコマンド共通):

- `--port` (env `RHINO_CLI_PORT`, デフォルト `50061`)
- `--host` (env `RHINO_CLI_HOST`, デフォルト `localhost`)
- `--timeout`, `--connect-timeout`
- `--pretty`, `--raw`
- `-q/--quiet`, `-v/--verbose`

### 主なサブコマンドの補足

- **`launch`** は Rhino を起動するだけ。プラグイン設定を書かず、RPC エンドポイントを待たない。`plugin set-port` (前) と `wait-ready` (後) を組み合わせること。`--restart` は起動前に Rhino を終了させる。デフォルトでは `-runscript _NoEcho` を渡してスタートウィンドウをスキップし、新規モデリングウィンドウを開いて `Rhino.RhinoDoc.ActiveDoc` を確定させる。スタートウィンドウを残したい場合のみ `--no-new-model` を指定する (この場合は `ActiveDoc` が `None` のままになり、プラグインのパネル / Python 操作が無音で失敗する点に注意)。`--script` を指定するとデフォルトのスタートアップスクリプトを上書きし、Rhino の `-runscript` 引数としてそのまま渡される。
- **`plugin set-port`** は macOS 上で `~/Library/Application Support/rhino-cli/RhinoCliPlugin/config.json` を書き込む。同梱プラグインは次回 Rhino ロード時にこれを読むため、`launch` (または Rhino 再起動) の前に呼ぶこと。`RhinoCli.Server` を組み込む第三者プラグインは独自にポート設定する。
- **`capabilities`** は AI エージェントと人間向けの自己説明 API。登録ハンドラ・パラメータ形状・結果形状・例・副作用・専用 CLI ラッパーを表示する。他ツールに構造化コンテキストを渡すときは `--format json|markdown|agent` を使う。
- **`call`** は登録済みハンドラ全般に対する汎用実行経路。
- **`screenshot`** は `screencapture` 経由で Rhino アプリケーションウィンドウを撮影する (macOS のみ)。プラグイン不要で動作する。`rhino-cli` を実行するターミナルプロセスに「画面収録」権限が必要。`--no-shadow` は影なしの密な画像、`--no-activate` は Rhino が既にフォーカスされているとき、`--window-id` は既知の macOS ウィンドウ ID を指定する場合に使う。
- **`capture-viewport`** はプラグインの `RhinoView.CaptureToBitmap` を経由してプロセス内で 1 ビューポートを撮影する。表示モード (`--mode`) は非破壊的に適用される。カメラ・投影・ズームはビューを変更し、撮影後に復元されない。`--camera`/`--target` と `--zoom-extents` は併用可能。`--out` を省略すると base64 PNG を含む JSON-RPC 結果が標準出力に出る。
- **`inspect-type`** は完全修飾名のみで解決する (サフィックス一致はしない)。短い名前から FQN を得るときは `search-types` と組み合わせる。`--binding public_static|public_instance|non_public|all` で可視性を切り替え、`--include-inherited` で基底メンバーまで辿る。ロード済み DLL の隣に `<AssemblyName>.xml` (例: `RhinoCommon.xml`) があれば XML ドキュメント要約が自動的に添付される。
- **`decompile-method`** は ICSharpCode.Decompiler を使う。コンストラクタには `.ctor` を渡す。オーバーロードがある場合は `--signature` に引数型名のカンマ区切りリストを渡す (`Point3d` でも `Rhino.Geometry.Point3d,bool` でも可)。形状とメソッド本体を 1 回の呼び出しで取りたいときは `inspect-type --with-body <method>` を使う。
- **`run-script`** は Rhino の `RunScript` 結果 JSON を表示する。`false` 返却で自動化を失敗させたい場合は `--fail-on-false` を使う。応答には `objects_added` / `objects_removed` / `command_prompt_changed` / `history_delta` が含まれる。
- **`probe-command`** は `! _-<Name> _Cancel` でコマンドを起動し、Rhino の現在ロケールで最初のプロンプトとオプションラベルを取得する。括弧内の ASCII ショートコード (例: `(D)`, `(P)`) はロケール非依存で、そのまま `_D` `_P` として送り返せる。即時副作用のあるコマンドや `-` (no-dialog) バリアントを持たないコマンドには注意して使うこと。

## ハンドラ

### 組込ハンドラ (RhinoCli.Server)

すべての `HandlerRegistry` に自動登録される:

| メソッド | 説明 |
|---------|------|
| `system.ping` | 生存確認。 |
| `system.version` | サーバ・プラグインのバージョン情報。 |
| `rpc.capabilities` | ハンドラメタデータのカタログ。 |
| `rpc.list_methods` | 登録済みメソッド名すべて。 |
| `rpc.list_plugins` | この接続経由で到達できるプラグインインスタンス。 |

### 同梱ハンドラ (RhinoCliPlugin)

同梱プラグインが起動時に登録する:

| メソッド | 説明 |
|---------|------|
| `rhino_cli.hello` | スモークテスト (`{hello:"world"}` を返す)。 |
| `rhino_cli.echo` | パラメータをそのまま返す。 |
| `rhino.run_script` | Rhino コマンドスクリプトを実行する。結果に `objects_added` / `objects_removed` / `command_prompt_changed` / `history_delta` を含む。 |
| `rhino.run_python` | `scriptcontext.doc` を事前配線したインライン Python を実行する。`result_expression` で戻り値を JSON シリアライズし、`stdout` と一緒に返す。 |
| `rhino.new_model` | 新しいアクティブドキュメントを作成する (テンプレート指定可)。 |
| `rhino.command_history` / `rhino.clear_command_history` | Eto `CommandHistoryViewModel` をリフレクションして Rhino コマンド履歴を読む / クリアする (`run_python` から触れない領域)。 |
| `rhino.list_commands` / `rhino.probe_command` | コマンド探索と動的プロンプト探査。`probe_command` はバックグラウンドスレッドからの `RhinoApp.SendKeystrokes("")` キャンセルが必要。 |
| `rhino.inspect_type` | `System.Reflection` 経由の API 探索。Rhino プロセス内にロードされた .NET 型のコンストラクタ・プロパティ・メソッド (オーバーロード単位)・イベント・フィールドを返す。 |
| `rhino.search_types` | ロード済みアセンブリ横断で型・メンバーを部分一致 (大小無視) 検索する。 |
| `rhino.decompile_method` | ICSharpCode.Decompiler で 1 メソッドを C# に逆コンパイルする。 |
| `rhino.capture_viewport` | 表示モード・カメラ・投影を構造化制御して 1 ビューポートを PNG 撮影する。`png_base64` と適用済みステートを返す。 |

### ハンドラセットを意図的に小さく保つ理由

同梱プラグインのハンドラ表面はライフサイクルと introspection に限定している。RhinoCommon Python の数行で書ける処理 (ファイル保存・オープン、形状追加、ID 指定でのオブジェクト列挙・削除、バッチ操作) は `rhino.run_python` + `result_expression` を使い、専用ハンドラを作らない。これによりプロトコルが `add_box → add_sphere → add_cylinder → …` のように膨れ上がるのを防ぐ。

境界ポリシーと `run_python` のレシピ集 (save / open / add / list / delete / capture) は `CLAUDE.md` と `docs/protocol.md` の `rhino.run_python` セクションにある。

## RhinoCliPlugin

同梱 Rhino プラグインのビルド:

```bash
dotnet build plugin/RhinoCliPlugin/RhinoCliPlugin.csproj
```

ビルド成果物は以下にコピーされる:

```text
~/Library/Application Support/McNeel/Rhinoceros/8.0/MacPlugins/RhinoCliPlugin
```

スモークテストシーケンス:

```bash
rhino-cli plugin set-port 50061
rhino-cli launch
rhino-cli wait-ready --port 50061 --timeout 120
rhino-cli capabilities --format agent --port 50061
rhino-cli call rhino_cli.hello --port 50061
rhino-cli new-model --port 50061
rhino-cli list-commands --pattern Box --port 50061
rhino-cli probe-command Box --port 50061
rhino-cli inspect-type Rhino.Geometry.Box --port 50061
rhino-cli search-types AddBox --port 50061
rhino-cli decompile-method Rhino.Geometry.Box ClosestPoint --signature Point3d --port 50061
rhino-cli run-script "_Zoom _Extents" --port 50061
rhino-cli history --tail 20 --port 50061
rhino-cli screenshot --out /tmp/rhino-cli-plugin.png
rhino-cli shutdown
```

## リリース

2 つのレジストリ、2 つのフロー。

### CLI → crates.io

1. `Cargo.toml` の `version` を上げる。
2. `cargo make publish-dry-run` (メタデータとパッケージ内容を検証する)。
3. `cargo publish` (事前に `cargo login` が必要)。

### RhinoCliPlugin → Yak

1. `plugin/RhinoCliPlugin/manifest.yml` の `version` を上げる (`RhinoCliPlugin.csproj` の `AssemblyVersion` / `FileVersion` も合わせる)。
2. `cargo make yak-build` — Release ビルド → manifest をビルド出力にコピー → `plugin/RhinoCliPlugin/bin/Release/net7.0/` に `*.yak` を生成する。
3. `cargo make yak-push` — 最新の `.yak` をアップロードする (初回のみ `yak login` が必要)。

Yak バイナリの場所は `YAK_BIN` 環境変数で上書きできる (デフォルト: `/Applications/Rhino 8.app/Contents/Resources/bin/yak`)。

## ドキュメント

- `docs/design.md` — アーキテクチャとスコープ
- `docs/protocol.md` — JSON-RPC 2.0 over TCP プロトコル
- `docs/plugin-integration.md` — 既存プラグインへの組込みガイド
- `docs/tasks.md` — 実装チェックリスト

## ライセンス

MIT
