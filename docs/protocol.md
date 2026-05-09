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
  "server": {"plugin_id":"RhinoCliPlugin","port":50099,"server_version":"0.1.0"},
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

(将来用、MVP では `pluginId` 1 個を要素 1 で返すだけ。) 同一 Rhino プロセス内に他の `RhinoCli.Server` インスタンスがある場合に発見可能にするため。MVP では協調機構は未実装で固定値を返す。

**Response**:
```json
{"plugins":[{"id":"GeoMLRhino","port":50061}]}
```

### 3.6 参照 Rhino automation メソッド

`plugin/RhinoCliPlugin` は rhino-cli 同梱のコアプラグインとして、AI エージェント用の `rhino.*` handler を登録する。`RhinoCli.Server` の built-in ではないため、既存プラグインへ組み込む場合は同等の handler を登録する。

| Method | Params | 説明 |
|--------|--------|------|
| `rhino.new_model` | `null` または `{ "template": "/path/to/template.3dm" }` | 新規 Rhino document を作成する |
| `rhino.run_script` | `{ "script": "...", "echo": false, "mru_display_string": null }` | Rhino command script を実行する |
| `rhino.command_history` | `{ "tail": 50 }` | Rhino history console text を取得する |
| `rhino.clear_command_history` | `null` | Rhino history console buffer を消去する |

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
