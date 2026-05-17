# rhino-cli プロトコル仕様

| 項目 | 内容 |
|------|------|
| ベース | JSON-RPC 2.0 (strict) |
| 転送 | JSON Lines over TCP (loopback) |
| 既定ポート | 50061 (プラグインごとに変えてよい) |
| エンコーディング | UTF-8 |

## 1. メッセージフォーマット

### 1.1 リクエスト

```json
{"jsonrpc":"2.0","id":1,"method":"system.ping","params":null}
```

| フィールド | 型 | 必須 | 説明 |
|----------|-----|-----|------|
| `jsonrpc` | string | ✓ | 必ず `"2.0"` |
| `id` | number \| string | ✓ (MVP では必須) | クライアントが採番。文字列でも数値でもよい |
| `method` | string | ✓ | `<namespace>.<method>` 形式推奨 |
| `params` | object \| array \| null | ✗ | 省略時 null 扱い |

> notification (id を省略する形) は MVP では未対応。サーバはエラーを返す。

### 1.2 成功レスポンス

```json
{"jsonrpc":"2.0","id":1,"result":{"pong":true}}
```

`result` の型は handler が自由に決める。

### 1.3 エラーレスポンス

```json
{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method not found","data":null}}
```

| フィールド | 説明 |
|----------|------|
| `code` | int (下表参照) |
| `message` | 短い人間向け説明 |
| `data` | 任意の追加情報 (省略可) |

`result` と `error` は排他。両方ある場合はクライアントが invalid response として扱う。

### 1.4 エラーコード

| Code | 区分 | 意味 |
|------|------|------|
| -32700 | spec | Parse error: JSON が壊れている |
| -32600 | spec | Invalid Request: 構造が JSON-RPC 2.0 違反 |
| -32601 | spec | Method not found |
| -32602 | spec | Invalid params |
| -32603 | spec | Internal error (handler 例外も含む) |
| -32000 ～ -32099 | server-defined | `RhinoCli.Server` 用 (将来予約。MVP では未使用) |
| その他 | アプリ固有 | プラグインが独自に定義可。負数を推奨 |

## 2. 転送層

### 2.1 フレーミング

- 1 メッセージ = 1 行 (JSON 中に LF を含めない)
- 改行コードは `\n` のみ (CRLF は使わない)
- 行末の余白は許容しない (パースは strict)

### 2.2 接続

- TCP loopback (127.0.0.1) のみ bind
- IPv6 は MVP 未対応
- TLS なし

### 2.3 接続ライフサイクル

CLI は **connect-per-call**:
1. TCP 接続
2. リクエスト 1 行送信
3. レスポンス 1 行受信
4. 切断

サーバは複数接続を受け付けるが、handler 実行は UI スレッド上でシリアライズされる。

> persistent/streaming は v0.2 以降。

## 3. 標準メソッド

`RhinoCli.Server` を組み込んだ全プラグインで自動的に提供される。

### 3.1 `system.ping`

接続確認用。

**Request**: params なし

**Response**:
```json
{"pong":true,"server":"GeoMLRhino","version":"0.1.0"}
```

| フィールド | 説明 |
|-----------|------|
| `pong` | 常に `true` |
| `server` | `TcpServer` コンストラクタに渡された `pluginId` |
| `version` | `RhinoCli.Server` のバージョン (handler 側で硬コード) |

### 3.2 `system.version`

サーバ・プロトコル両方のバージョンを返す。

**Response**:
```json
{"protocol":"jsonrpc-2.0","server":"0.1.0","plugin":"GeoMLRhino"}
```

### 3.3 `rpc.list_methods`

このサーバで利用可能な全メソッド名 (system / rpc / plugin 全て) を返す。互換用の軽量 API。handler 仕様は `rpc.capabilities` を使う。

**Response**:
```json
{"methods":["geoml.durability_test","rpc.capabilities","rpc.list_methods","rpc.list_plugins","system.ping","system.version"]}
```

### 3.4 `rpc.capabilities`

登録済み handler の説明、params/result schema、例、専用 CLI、side effects を返す。`params.method` 指定時は 1 handler のみ返す。

**Request**:
```json
null
```

または:

```json
{"method":"rhino.run_script"}
```

**Response**:
```json
{
  "server": {"plugin_id":"RhinoCliPlugin","port":50061,"server_version":"0.1.0"},
  "methods": [
    {
      "method": "rhino.run_script",
      "description": "Run a Rhino command script on the Rhino UI thread.",
      "paramsSchema": "{ script: string, echo?: boolean, mru_display_string?: string }",
      "resultSchema": "{ status: string, success: boolean, script: string, command_prompt: string }",
      "examples": ["rhino-cli run-script \"_Zoom _Extents\""],
      "dedicatedCommand": "rhino-cli run-script <SCRIPT>",
      "sideEffects": "Executes Rhino commands and may modify the active document.",
      "category": "rhino"
    }
  ]
}
```

### 3.5 `rpc.list_plugins`

(将来用、MVP では `pluginId` 1 個を要素 1 で返すだけ。) 同一 Rhino プロセス内に他の `RhinoCli.Server` インスタンスがある場合に発見可能にするため。MVP では協調機構は未実装で固定値を返す。専用 CLI: `rhino-cli list-plugins`。

**Response**:
```json
{"plugins":[{"id":"GeoMLRhino","port":50061}]}
```

### 3.6 参照 Rhino automation メソッド

`plugin/RhinoCliPlugin` は rhino-cli 同梱のコアプラグインとして、AI エージェント用の `rhino.*` handler を登録する。`RhinoCli.Server` の built-in ではないため、既存プラグインへ組み込む場合は同等の handler を登録する。

ハンドラセットは意図的に最小化されている。RhinoCommon を直接叩けば実装できるもの（box 追加、object 列挙、保存、削除、ビューポート画像化など）は専用ハンドラを増やさず `rhino.run_python` の `result_expression` 経由で行う。境界ポリシーは `CLAUDE.md` の「ハンドラ追加の境界ポリシー」を参照。

| Method | Params | 説明 |
|--------|--------|------|
| `rhino.new_model` | `null` または `{ "template": "/path/to/template.3dm" }` | 新規 Rhino document を作成する |
| `rhino.run_script` | `{ "script": "...", "echo"?: bool, "mru_display_string"?: string }` | Rhino command script を実行する。result は `objects_added` / `objects_removed` / `command_prompt_changed` / `history_delta` を含むので、コマンドが実際に何かしたかを呼び出し側が判定できる |
| `rhino.run_python` | `{ "source": "...", "result_expression"?: "..." }` | Python source 文字列をインライン実行する。`scriptcontext.doc` は active doc にセット済み。`print()` 出力は `stdout` に捕獲。`result_expression` を渡すと source 実行後にその式を評価し、戻り値を JSON シリアライズして `result` に格納する（プリミティブはそのまま、複合型は `System.Text.Json` を試み失敗時は `repr()` 文字列）。本ハンドラはジオメトリ生成・属性操作・RhinoCommon 直叩きの公式エスケープハッチ |
| `rhino.command_history` | `{ "tail": 50 }` | Rhino history console text を取得する |
| `rhino.clear_command_history` | `null` | Rhino history console buffer を消去する |
| `rhino.list_commands` | `null` または `{ "pattern": "Box", "include_unloaded": false }` | Rhino に登録済みのコマンド名一覧を返す。AI agent のコマンド発見用 |
| `rhino.probe_command` | `{ "name": "Box" }` | コマンドを `! _-{Name} _Cancel × 5` で起動・即時中断し（300ms 経過時には background thread から `RhinoApp.SendKeystrokes("")` で Esc も送出）、最初の Get プロンプトを `RhinoApp.CommandPrompt` から、Write/WriteLine 出力を `RhinoApp.CapturedCommandWindowStrings` から捕獲して返す。option short code は `(D)` `(P)` 等 ASCII 安定で `_D` `_P` としてそのまま渡せる |
| `rhino.inspect_type` | `{ "name": "Rhino.Geometry.Box", "binding"?: "public" \| "public_instance" \| "public_static" \| "non_public" \| "all", "include_inherited"?: bool }` | `System.Reflection` でロード済みの .NET 型を内省し、constructors / properties / methods（オーバーロードはグルーピング）/ events / fields を構造化 JSON で返す。型解決は FQN のみ（末尾一致なし）。`run_python` で RhinoCommon を呼ぶ前の API 発見用。詳細は §3.2.1 |
| `rhino.search_types` | `{ "pattern": "AddBox", "scope"?: "all" \| "types" \| "members", "assembly"?: string, "limit"?: int }` | ロード済みアセンブリから型 / メンバ名の部分一致 (case-insensitive) を検索する。`inspect_type` 前段の FQN 解決用。デフォルトは `Rhino*` / `RhinoCommon` / `RhinoCli*` 配下に絞り込む。詳細は §3.2.2 |

#### 3.2.1 `rhino.inspect_type` の詳細

`run_python` で AI が知らない型を扱う前に、constructor のオーバーロードや
property の型を事前確認するための発見ハンドラ。`System.Reflection` で
Rhino プロセスにロード済みのアセンブリを直接 reflection するので、
プラグインが追加した型もそのまま対象になる。

**型解決ポリシー**: `Type.GetType(name)` → 失敗時は
`AppDomain.CurrentDomain.GetAssemblies()` を巡って `Assembly.GetType(name)`。
**末尾一致フォールバックは行わない** (`Box` だけで `Rhino.Geometry.Box`
を解決する等)。短い名前から FQN を引きたい場合はまず `rhino.search_types`
（Phase C で追加予定）を使う。

**binding パラメータ**:

| 値 | 含まれるメンバ |
|----|----------------|
| `"public"` (デフォルト) | Public Instance + Public Static |
| `"public_instance"` | Public Instance のみ |
| `"public_static"` | Public Static のみ |
| `"non_public"` | NonPublic + Public, Instance + Static |
| `"all"` | NonPublic + Public, Instance + Static |

**`include_inherited`**: デフォルトは `false` (DeclaredOnly)。`true` で
親クラスのメンバも結果に含める。constructors は常に DeclaredOnly。

**結果スキーマ抜粋**:

```jsonc
{
  "full_name": "Rhino.Geometry.Box",
  "assembly": "RhinoCommon",
  "kind": "struct",          // "class" | "struct" | "interface" | "enum"
  "is_abstract": false,
  "is_sealed": false,
  "base_type": "System.ValueType",
  "interfaces": ["..."],
  "summary": "Represents the value of a plane and three intervals...",
  "constructors": [
    {
      "params": [{"name": "plane", "type": "Rhino.Geometry.Plane", "summary": "...", ...}, ...],
      "is_public": true,
      "summary": "Initializes a new instance of the Box class..."
    }
  ],
  "properties": [
    { "name": "Center", "type": "Rhino.Geometry.Point3d", "get": true, "set": false, "static": false, "summary": "Gets the center point of the box." }
  ],
  "methods": [
    {
      "name": "PointAt",
      "static": false,
      "overloads": [
        {
          "params": [...],
          "return_type": "Rhino.Geometry.Point3d",
          "is_generic": false,
          "generic_args": [],
          "summary": "Evaluates the Box at the given parameters...",
          "returns": "the evaluated point"
        }
      ]
    }
  ],
  "events": [{"name": "...", "handler_type": "...", "summary": "..."}],
  "fields": [{"name": "...", "type": "...", "static": false, "is_literal": false, "summary": "..."}]
}
```

**param オブジェクト**:

```jsonc
{
  "name": "plane",
  "type": "Rhino.Geometry.Plane",
  "is_out": false,
  "is_ref": false,
  "has_default": false,
  "default_value": null,
  "summary": "the plane on which to base the box"
}
```

**XML ドキュメントの取り込み (Phase B)**: ハンドラは型の所属アセンブリの
隣にある `<AssemblyName>.xml` ファイル (例: `RhinoCommon.xml`) を自動で
読み込み、各メンバの `<summary>` / `<param>` / `<returns>` を JSON の
`summary` / param オブジェクトの `summary` / overload の `returns` に
attach する。XML が存在しない、メンバ ID が見つからない場合は空文字列を
返す（エラーにはならない）。RhinoCommon の XML は通常英語のみ同梱なので、
日本語ロケールでも summary は英語で返る。詳細な lookup 規約は
`docs/plugin-integration.md` の「XML doc lookup」を参照。

#### 3.2.2 `rhino.search_types` の詳細

`inspect_type` は FQN（完全修飾名）でしか型を解決しないため、AI が短い
名前しか知らないときに **FQN を引くためのインデックス検索ハンドラ**。
全 ロード済み アセンブリを `AppDomain.CurrentDomain.GetAssemblies()` で
巡って、型名およびパブリックメンバ名に対して部分一致（case-insensitive）で
ヒットを返す。

**パラメータ**:

| 名前 | 既定値 | 説明 |
|------|--------|------|
| `pattern` | 必須 | 部分一致される文字列。空はエラー (-32602) |
| `scope` | `"all"` | `"all"` / `"types"` / `"members"` |
| `assembly` | 未指定 | アセンブリ名を完全一致で指定するとそれのみ対象。未指定時は `Rhino*` / `RhinoCommon` / `RhinoCli*` に絞り込み |
| `limit` | 50 | 最大マッチ数。超えた場合 `truncated: true` |

**`type.IsVisible`** で外部から見えない internal 型は除外する。`MethodBase`
の `IsSpecialName` (= property/event accessor) も除外。

**結果スキーマ**:

```jsonc
{
  "matches": [
    { "kind": "type", "full_name": "Rhino.Geometry.Box", "member": null, "assembly": "RhinoCommon" },
    { "kind": "method", "full_name": "Rhino.DocObjects.Tables.ObjectTable", "member": "AddBox", "assembly": "RhinoCommon" }
  ],
  "truncated": false
}
```

`kind` は `"type" | "method" | "property" | "field" | "event" | "constructor" | "member"`。
member kinds の `full_name` は **DeclaringType の FQN**、`member` は
メンバ名。type kind の `member` は `null`。

**典型ワークフロー**:

1. `search_types AddBox` → `ObjectTable.AddBox` (method) と `Box` (type) などが返る
2. AI が `inspect_type Rhino.DocObjects.Tables.ObjectTable` を呼んで overload を確認
3. シグネチャに合わせて `run_python` でコードを書く

#### `rhino.run_python` の代表レシピ

専用ハンドラを置かない代わりに、よく使うパターンは `source` + `result_expression` で組む。

**ドキュメント保存**
```json
{ "source": "import scriptcontext as sc, Rhino.FileIO as fio\nopts = fio.FileWriteOptions()\nopts.FileVersion = 8\nopts.SuppressDialogBoxes = True\nok = sc.doc.WriteFile('/tmp/x.3dm', opts)",
  "result_expression": "ok" }
```

**Box 追加（軸並行 / 傾斜は Plane と Interval を組む）**
```json
{ "source": "import scriptcontext as sc, Rhino.Geometry as rg\nbb = rg.BoundingBox(rg.Point3d(0,0,0), rg.Point3d(100,100,100))\ng = sc.doc.Objects.AddBox(rg.Box(bb))\nsc.doc.Views.Redraw()",
  "result_expression": "str(g)" }
```

**bbox / type フィルタつきオブジェクト列挙**
```json
{ "source": "import scriptcontext as sc, json\nrows = []\nfor o in sc.doc.Objects:\n    if o is None: continue\n    bb = o.Geometry.GetBoundingBox(True)\n    rows.append({'id':str(o.Id),'type':str(o.ObjectType),'min':[bb.Min.X,bb.Min.Y,bb.Min.Z],'max':[bb.Max.X,bb.Max.Y,bb.Max.Z]})",
  "result_expression": "json.dumps(rows)" }
```

**ID 指定削除**
```json
{ "source": "import scriptcontext as sc, System\nids = ['<guid>']\ndeleted = sum(1 for s in ids if sc.doc.Objects.Delete(System.Guid.Parse(s), True))\nsc.doc.Views.Redraw()",
  "result_expression": "deleted" }
```

**ビューポート PNG 出力**
```json
{ "source": "import scriptcontext as sc, System.Drawing as sd, System.Drawing.Imaging as sdi\nbmp = sc.doc.Views.ActiveView.CaptureToBitmap(sd.Size(1280, 720))\nbmp.Save('/tmp/v.png', sdi.ImageFormat.Png)\nbmp.Dispose()",
  "result_expression": "True" }
```

## 4. プラグイン固有メソッド命名規約

| Pattern | 用途 | 例 |
|---------|------|-----|
| `<plugin>.<verb>` | 操作 | `geoml.durability_test` |
| `<plugin>.<verb>_<resource>` | 個別操作 | `geoml.create_extrude` |
| `<plugin>.<resource>.<verb>` | リソース指向 | `geoml.document.snapshot` |

`system` `rpc` の 2 つは予約。プラグインは使ってはならない。

## 5. params の慣習

### 5.1 推奨

- 名前付き params (`object`) を優先
- positional params (`array`) は引数 0–2 個の単純メソッドのみ
- 空 params は `null` または `{}` を許容

### 5.2 大きな入力

- 1 リクエストあたり 1 MiB 以下を推奨
- それ以上はファイルパス渡しまたは別の data ingestion チャネルを設ける (handler 設計で判断)

## 6. エラー応答の追加データ

`error.data` 推奨フィールド:

```json
{
  "code": -32602,
  "message": "Invalid params",
  "data": {
    "field": "scenarios",
    "expected": "array of strings",
    "got": "null"
  }
}
```

ただし `data` は厳密にはオプション。CLI は表示するが意味は解釈しない。

## 7. 互換性ポリシー

- メジャーバージョン (`0.1` → `1.0`) のみで破壊的変更を許可
- 0.x の間は最小限の互換維持に留める (early stage)
- 標準メソッド (`system.*`, `rpc.*`) のシグネチャ変更は破壊的変更扱い
- プラグイン固有メソッドは各プラグインのバージョニングに従う (rhino-cli は関与しない)

## 8. テストベクター

### 8.1 ping

```
→ {"jsonrpc":"2.0","id":1,"method":"system.ping"}
← {"jsonrpc":"2.0","id":1,"result":{"pong":true,"server":"GeoMLRhino","version":"0.1.0"}}
```

### 8.2 method not found

```
→ {"jsonrpc":"2.0","id":2,"method":"foo.bar","params":null}
← {"jsonrpc":"2.0","id":2,"error":{"code":-32601,"message":"Method not found"}}
```

### 8.3 parse error

```
→ {"jsonrpc":"2.0","id":3,"meth
← {"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}}
```

### 8.4 invalid request (missing method)

```
→ {"jsonrpc":"2.0","id":4}
← {"jsonrpc":"2.0","id":4,"error":{"code":-32600,"message":"Invalid Request"}}
```

### 8.5 invalid params (handler が型ミスを検出)

```
→ {"jsonrpc":"2.0","id":5,"method":"geoml.durability_test","params":{"scenarios":42}}
← {"jsonrpc":"2.0","id":5,"error":{"code":-32602,"message":"Invalid params","data":{"field":"scenarios","expected":"array"}}}
```

これら 5 つを統合テスト (`tests/e2e_mock.rs`) のミニマムスイートとして必ず通すこと。
