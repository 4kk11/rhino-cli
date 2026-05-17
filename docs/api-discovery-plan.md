# RhinoCommon API 発見機構の統合プラン

| 項目 | 内容 |
|------|------|
| 作成日 | 2026-05-17 |
| ステータス | Phase C 完了 (Phase D 待ち) |
| 関連 | `docs/design.md`, `docs/protocol.md`, `docs/tasks.md` |

---

## 1. 意図 (なぜ必要か)

AI が `rhino.run_python` で RhinoCommon を直接叩いて自律的にモデリングする
ためには、AI が「知らない API を自分で発見できる」能力が不可欠である。
RhinoCommon は巨大 (1,524 ファイル) で、AI の事前知識だけでは
constructor のオーバーロード、property の型、method のシグネチャを
正確に呼べない。

現状の発見手段:

- 外部 MCP `mcpeek` — DLL を静的解析して C# を復元する
- `run_python` 内での `dir()` / `inspect.getmembers()`
- 失敗時の traceback を読む

問題:

- `mcpeek` は **外部依存**で、rhino-cli を導入したユーザーが別途設定が
  必要。rhino-cli 単体での自律操作という主張が崩れる
- `dir()` は method の overload や引数型を構造化で返せず、AI が毎回
  IronPython specific な reflection スクリプトを書く必要がある
- traceback だけでは「正しい呼び方」を能動的に発見できない

→ rhino-cli (RhinoCliPlugin) 自身に **動的 reflection ベースの API
発見ハンドラ**を組み込み、AI が rhino-cli だけで API 発見 → 実行 →
検証のループを閉じられるようにする。

## 2. なぜ reflection ベースか (選択理由)

候補 A: ICSharpCode.Decompiler を C# 側に組み込み、mcpeek 同等の DLL
decompile を統合。
候補 B: `System.Reflection` で実機にロード済みの型を内省。
候補 C: 静的に bundled な JSON スキーマを同梱。

**B を主軸に選ぶ。理由:**

1. **実機が真実**: RhinoCliPlugin は Rhino プロセス内で動いており、
   RhinoCommon は AppDomain にロード済み。reflection で取得する情報は
   現在走っているバージョンの実体に一致する
2. **追加依存ゼロ**: `System.Reflection` は .NET 標準。配布物が膨らまない
3. **プラグイン追加型もカバー**: 静的 DLL 解析と違い、他のプラグインが
   提供する型も自動でカバーできる
4. **構造化 JSON で返せる**: AI が parse しやすい

ただし、reflection は **メソッド本体 (IL)** を C# で復元できない。これは
ICSharpCode.Decompiler が必要。AI が API を呼ぶだけなら body は通常不要
だが、「このメソッドは内部で何を呼んでいるか」を読みたい場面がある
(例: コマンドの挙動推測、edge case のデバッグ)。

→ **body 復元はオプション handler として A も併設**。NuGet
`ICSharpCode.Decompiler` を追加する。

候補 C は **却下**: スキーマ snapshot がバージョン乖離する管理コストが
重い。動的解析の利点を失う。

## 3. ゴール / 非ゴール

### ゴール

- RhinoCliPlugin に API 発見用ハンドラを 3 つ追加
  - `rhino.inspect_type` — 型の構造化された詳細
  - `rhino.search_types` — 名前検索 (type/member 横断)
  - `rhino.decompile_method` — メソッド本体の C# 復元 (オプション)
- `RhinoCommon.xml` (XML doc) を読み取り、`<summary>` を結果に含める
- 各ハンドラに dedicated CLI を提供
- `docs/protocol.md` の `rhino.run_python` セクションに
  「API 発見プレイブック」を追加し、AI が順序立てて発見動作を取れるようにする
- 既存 `mcpeek` 依存を AI ワークフローから外せる (任意の補助として残す
  ことは可)

### 非ゴール

- `run_python` のエラーメッセージへの hint 自動埋め込み (別タスクで検討)
- 全 .NET assembly の永続インデックス (毎回 reflection で十分速い前提)
- decompile キャッシュ管理 (ICSharpCode.Decompiler のインスタンス再利用
  のみ。永続化はしない)

## 4. ハンドラ仕様 (案)

### 4.1 `rhino.inspect_type`

**目的**: 特定の型 (class / struct / enum / interface) の members を
構造化 JSON で返す。

```jsonc
// Params
{
  "name": "Rhino.Geometry.Box",
  // 任意。デフォルト Public Instance + Public Static
  "binding": "public" | "public_static" | "non_public" | "all",
  // 任意。継承元のメンバを含むか
  "include_inherited": false
}

// Result
{
  "full_name": "Rhino.Geometry.Box",
  "assembly": "RhinoCommon",
  "kind": "struct" | "class" | "interface" | "enum",
  "base_type": "System.ValueType",
  "interfaces": ["System.ICloneable", ...],
  "summary": "<XML doc summary if available>",
  "constructors": [
    {
      "params": [
        {"name": "plane", "type": "Rhino.Geometry.Plane"},
        {"name": "xSize", "type": "Rhino.Geometry.Interval"},
        ...
      ],
      "summary": "<XML doc>"
    }
  ],
  "properties": [
    {"name": "Center", "type": "Point3d", "get": true, "set": false, "static": false, "summary": "..."}
  ],
  "methods": [
    {
      "name": "PointAt",
      "static": false,
      "overloads": [
        {"params": [...], "return_type": "Point3d", "summary": "..."}
      ]
    }
  ],
  "events": [...],
  "fields": [...]
}
```

**実装メモ**:
- `Type.GetConstructors(BindingFlags)`, `GetMethods`, `GetProperties`,
  `GetEvents`, `GetFields` を BindingFlags に従って収集
- 型名の解決は **完全修飾名 (FQN) のみ**:
  ① `Type.GetType(name, false)` を試す
  ② 失敗時は `AppDomain.CurrentDomain.GetAssemblies()` を巡って
     `GetType(name, false)` を試す
  ③ それでも見つからなければエラー (`-32602 / type_not_found`)
- **末尾一致フォールバックは採用しない** (`Box` だけで `Rhino.Geometry.Box`
  を解決する等)。`Rhino.Geometry.Box` と `Rhino.UI.Box` のような衝突で
  誤った型を返すリスクが高い。短い名前から型を見つけたいときは
  `rhino.search_types` で先に FQN を解決させる運用にする

### 4.2 `rhino.search_types`

**目的**: ロード済み assembly 全体から型名 / メンバ名を検索。

```jsonc
// Params
{
  "pattern": "AddBox",         // 部分一致 (大文字小文字無視)
  "scope": "types" | "members" | "all",  // デフォルト "all"
  "assembly": "RhinoCommon",   // 任意。assembly 名で絞り込み
  "limit": 50                  // デフォルト 50
}

// Result
{
  "matches": [
    {"kind": "type", "full_name": "Rhino.Geometry.Box", "assembly": "RhinoCommon"},
    {"kind": "method", "type": "Rhino.DocObjects.Tables.ObjectTable", "name": "AddBox", "assembly": "RhinoCommon"},
    ...
  ],
  "truncated": false
}
```

**実装メモ**:
- assembly フィルタなしの場合、`Rhino*`, `RhinoCommon`, `RhinoCli*`
  のみをデフォルト対象に (System.* / Microsoft.* を除外しないと結果が
  膨大になる)
- `BindingFlags.Public | Static | Instance` を対象
- regex 対応は v2 検討。MVP は **部分一致 (case-insensitive)** のみ

### 4.3 `rhino.decompile_method` (オプション)

**目的**: 特定メソッドの本体を C# 復元して返す。`mcpeek` の主要利点
(body 可視化) を統合する。

```jsonc
// Params
{
  "type": "Rhino.Geometry.Box",
  "method": "ClosestPoint",
  "signature": "(Point3d)"  // 任意。overload 特定
}

// Result
{
  "type": "Rhino.Geometry.Box",
  "method": "ClosestPoint",
  "signature": "(Rhino.Geometry.Point3d)",
  "csharp": "public Point3d ClosestPoint(Point3d testPoint) { ... }"
}
```

**実装メモ**:
- NuGet `ICSharpCode.Decompiler` を追加 (現行版 8.x、.NET Standard 2.0
  互換、Rhino 8 の plugin runtime と整合)
- `CSharpDecompiler` を **assembly ごとにキャッシュ** (再構築コスト大)。
  ただしプロセス停止までで OK、永続化なし
- decompile 失敗 (obfuscated, native call, generic instantiation) は
  graceful に `error` フィールドで返す

### 4.4 XML ドキュメント取り込み

- `RhinoCommon.xml` を plugin と同じディレクトリから読む (`.dll` の
  隣に `.xml` がある慣習)
- 初回参照時にロードしてメモリにキャッシュ
- 取得した `<summary>` テキスト (XML タグ除去・整形) を `inspect_type`
  / `decompile_method` の各 member の `summary` に添付
- RhinoCommon 以外の assembly はベストエフォート (xml なしなら空欄)

## 5. CLI subcommand

CLI レベルでは **リフレクション (シグネチャ) と デコンパイル (body) を
分離** する。デフォルトはリフレクションのみで軽量に動き、body が欲しい
場合だけ明示的にオプション / 別 subcommand を使う。

### 5.1 デフォルト = リフレクションのみ

- `rhino-cli inspect-type <FQN> [--binding ...] [--include-inherited]`
  - `rhino.inspect_type` を呼ぶだけ。body は含まれない
  - 出力には method の `name` / `overloads` / `params` / `return_type` /
    `summary` のみ。`body` フィールドは付かない
- `rhino-cli search-types <PATTERN> [--scope ...] [--assembly ...] [--limit N]`
  - `rhino.search_types` を呼ぶだけ

### 5.2 body が欲しいとき = オプション / 別 subcommand

- `rhino-cli decompile-method <TYPE_FQN> <METHOD> [--signature ...]`
  - `rhino.decompile_method` を単発で呼ぶ。特定 1 メソッドの body だけ
    欲しい場合の最短経路
- `rhino-cli inspect-type <FQN> --with-body <METHOD>` (任意、複数指定可)
  - 内部で `inspect_type` を呼んだあと、指定 method について
    `decompile_method` を追加で呼んで結果の `methods[*].overloads[*]` に
    `body` フィールドを merge して返す
  - リフレクション情報と body を一括で取りたい場面の利便性のため

これらは既存の Rust CLI ルーティング (`call <method>` の薄いラッパ) に
従い、`src/commands/` 配下に追加。`--with-body` は Rust CLI 側のオプション
合成で実装し、handler 自体は分離したまま (`inspect_type` は decompile
を実行しない)。

## 6. ドキュメント追従 (必須)

ソース変更と同じ作業内で更新する (CLAUDE.md ルール):

- `README.md` — 「API 発見」セクションを追加、各 dedicated CLI を表に
- `docs/design.md` — 新 handler の責務と reflection ベース選択の根拠
- `docs/protocol.md` — 3 handler の schema 詳述、「API 発見
  プレイブック」セクション追加 (`mcpeek` 外部依存は撤去、内蔵 3 ハンドラ
  + `dir()` + traceback の順序を明示)
- `docs/plugin-integration.md` — XML doc 読込ポリシー、ICSharpCode.Decompiler
  依存追加の注意
- `plugin/RhinoCliPlugin/README.md` — handler 追加
- `Makefile.toml` — テスト追加分の連結があれば
- `docs/tasks.md` — 進捗 (Phase X.Y) 追加

## 7. テスト方針

### C# (xUnit, `RhinoCli.Server.Tests`)

- `InspectTypeHandler`: System 標準型 (`System.String`) で
  smoke test (Rhino 依存なし)
- `SearchTypesHandler`: 自前テスト assembly に target 型を作って検索
- XML doc parser の単体テスト (整形ロジック)
- decompile は ICSharpCode.Decompiler のスモークテスト 1 本

### 実機検証 (手動)

- `rhino-cli inspect-type Rhino.Geometry.Box` で constructor 5 件以上が
  取れる
- `rhino-cli search-types AddBox` で `ObjectTable.AddBox` が見つかる
- XML summary が日本語/英語のどちらでも壊れず取れる
- `rhino-cli decompile-method Rhino.Geometry.Box ClosestPoint` で
  読める C# が返る

## 8. 段階的実装ステップ

各 Phase で **同時に関連 docs を更新** する (CLAUDE.md ルール: 別タスクに
回さない)。Phase ごとに commit を分け、gitmoji + 英語 + 署名なしで
コミットする。

| Phase | 内容 | 同時に更新する docs | 完了基準 |
|------|------|--------------------|---------|
| A | `InspectTypeHandler` (FQN 解決のみ、XML なし) + 単体テスト + Rust CLI `inspect-type` | `docs/protocol.md`, `README.md`, `plugin/RhinoCliPlugin/README.md`, `docs/design.md` | `rhino-cli inspect-type System.String` で members 取得 |
| B | XML doc loader + `inspect_type` の `summary` 添付 | `docs/protocol.md`, `docs/plugin-integration.md` | `Rhino.Geometry.Box` の `summary` に英語サマリが入る |
| C | `SearchTypesHandler` + 単体テスト + Rust CLI `search-types` | `docs/protocol.md`, `README.md`, `plugin/RhinoCliPlugin/README.md` | `rhino-cli search-types AddBox` で `ObjectTable.AddBox` がヒット |
| D | NuGet `ICSharpCode.Decompiler` 追加 + `DecompileMethodHandler` + 単体スモーク + Rust CLI `decompile-method` | `docs/protocol.md`, `docs/plugin-integration.md`, `README.md`, `plugin/RhinoCliPlugin/README.md`, `docs/design.md` | `rhino-cli decompile-method Rhino.Geometry.Box ClosestPoint` で C# 復元成功 |
| E | Rust CLI `inspect-type --with-body <METHOD>` オプション合成 | `README.md`, `docs/protocol.md` (CLI 例の追記のみ) | `inspect-type --with-body ClosestPoint` で body 入りの統合結果が返る |
| F | `docs/protocol.md` に「API 発見プレイブック」セクション追加 + `mcpeek` 言及を全 docs から削除 | `docs/protocol.md`, `README.md`, 既存 docs 全 grep | AI が順序立てた発見動作を取れる文書が完成 |
| G | 実機 e2e 確認 | `docs/tasks.md` に Phase 進捗 + 検証メモ | §7 の検証 4 件すべて通る |

**MVP の範囲**: Phase A 〜 E。F は文書整備、G は検証。本機能の AI 利用に
必要な実装は E までに揃う。

## 9. CLAUDE.md 境界ポリシーへの照合

> 1. **run_python では実装困難**

- `inspect_type`: `dir()` で attribute は取れるが、**method overload
  ごとの param 型** は IronPython specific な `Type.GetMethods` を毎回
  書く必要があり、構造化 JSON を組むスクリプトは 50 行超。AI が毎回書く
  には高コスト → △ 厳密には書けるが非効率
- `search_types`: 全 assembly 横断検索は Python では非常に煩雑 → ◯
- `decompile_method`: NuGet ライブラリが必要 → ◯

> 2. **概念単位で 1 個に収束**: ✓ 各 handler が 1 概念
> 3. **構造化 I/O が本質的**: ✓ JSON で型情報を返すのが本質

**結論**: 3 handler とも追加に値する。(1) は inspect_type のみ若干弱い
が、AI の発見効率の観点で正当化可能。

## 10. レビュー確定事項 (2026-05-17)

1. **handler 名**: `rhino.inspect_type` / `rhino.search_types` /
   `rhino.decompile_method` で確定
2. **末尾一致フォールバック**: **採用しない** (FQN のみ受理)。衝突リスク
   回避
3. **decompile を MVP に入れる**: ✓ Phase D に組み込み
4. **`mcpeek` の扱い**: docs から完全に外す。Phase F で全 docs を grep
   して削除
5. **docs 更新タイミング**: 各 Phase ごとに小刻みに更新

## 11. オープンな実装懸念 (実装着手時に判断)

1. **XML doc の言語**: RhinoCommon の `.xml` は通常英語のみ同梱。日本語
   ロケールでも英語 summary が返る。これは AI には問題なし (むしろ英語の
   方が正確)
2. **decompile のパフォーマンス**: CSharpDecompiler はメソッド単位だと
   それなりに速いが、assembly 全体の type system 構築でメモリを使う
   (数百 MB 級)。プラグインの常駐メモリが膨らむのを許容するか、初回
   呼び出しまで遅延ロードするか → **遅延ロード + assembly 単位キャッシュ**
   を Phase D で実装し、e2e で計測
3. **`inspect_type` の `BindingFlags`**: デフォルトを「Public Instance +
   Public Static」とする。多くの factory メソッドは static なので両方
   含める方が AI フレンドリー (`binding="public"` パラメータで明示変更可)
4. **`search_types` のデフォルト assembly フィルタ**: `System.*` /
   `Microsoft.*` を除外しないと結果が膨大になる。デフォルト対象は
   `Rhino*`, `RhinoCommon`, `RhinoCli*` とし、`--assembly` で拡張可能
