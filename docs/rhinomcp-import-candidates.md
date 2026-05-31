# RhinoMCP から輸入する機能の判断

`references/RhinoMCP`（McNeel 公式 Rhino MCP Platform）の良い点のうち、rhino-cli に輸入すべきものを選定する。詳細な機能比較は [comparison-rhinomcp.md](comparison-rhinomcp.md) を参照。

## 判断軸（rhino-cli の境界ポリシー）

輸入可否は「便利かどうか」ではなく、rhino-cli の設計原則で判定する：

1. **run_python で書けるものは handler 化しない**。書けないもの（UI スレッド制御・特権操作・reflection・プロセス制御・クラッシュログ解析など）だけが handler 候補。
2. **薄い汎用基盤を保つ**。概念単位で1個に収束し、形状/パラメータ次元で分裂しない。
3. **構造化 I/O が本質的に意味を持つ**こと。

この軸だと RhinoMCP の object/scene/material/camera 系 tool は「輸入しない（= run_python レシピで足りる）」が一貫した答えになる。輸入価値があるのは、rhino-cli の弱点（C# 実行・診断・利用体験・GH）を埋めるものに絞られる。

---

## 結論（優先度順）

| # | 輸入対象 | 判定 | 理由（境界ポリシー） | 規模 |
|---|---|---|---|---|
| 1 | **run_csharp**（C# スクリプト実行 handler） | ✅ 輸入する | run_python の双子。「自分自身がエスケープハッチ」枠。1概念に収束、薄い | 小 |
| 2 | **クラッシュログ解析を doctor に統合** | ✅ 輸入する | ログ解析は run_python 不可。診断概念で収束。AI 自律切り分けに直結 | 小〜中 |
| 3 | **診断 Resource（host env / installed plugins）** | ✅ 輸入する | 既存 `doctor`/`capabilities` の自然な延長。run_python では取りづらい環境情報 | 小 |
| 4 | **同梱 Claude Code エージェント/スキル** | ✅ 輸入する | コード本体の薄さに無影響。capabilities 駆動と親和。利用体験を一気に底上げ | 中 |
| 5 | **list_objects 相当の「フィルタ付き一覧」を run_python レシピ化** | ✅ 輸入する（handlerではなくレシピ） | handler 化は境界違反。だが頻用パターンなので protocol.md に1行レシピで提供 | 小 |
| 6 | **Grasshopper サポート（最小核のみ）** | ⚠ 条件付き | canvas 操作は run_python 困難 → handler 候補だが分裂リスク大。最小核 or 別プラグイン分離が前提 | 大 |
| 7 | **MCP サーバ frontend（stdio）** | ⚠ 条件付き | CLI と直交。MCP クライアント直結の価値は大きいが MVP スコープ判断が要る | 中〜大 |
| 8 | **slot / マルチインスタンス router** | ❌ 当面見送り | 価値はあるが launch/shutdown 責務分離を持つ現設計への大改造。MVP には過剰 | 特大 |
| — | object/selection/material/camera/zoom/open/save の構造化 tool | ❌ 輸入しない | run_python で完結。境界ポリシーで明確に NG（レシピ化で代替） | — |

---

## 輸入する（採用）

### 1. run_csharp — C# スクリプト実行 handler ⭐最優先

**何**: RhinoMCP の `run_csharp`（`rhino/plugin/Tools/RunCSharpTool.cs`）相当。`__rhino_doc__` を注入し、stdout/error を構造化して返す。

**なぜ輸入が正当か**:
- rhino-cli の境界ポリシーは「`run_python` ✓: 自分自身がエスケープハッチ」を明示的に許容している。`run_csharp` はその C# 版で、同じ理由で正当。
- C# は RhinoCommon に型安全・高速にアクセスでき、Python より「internal 叩き・厳密な型操作・大きめのロジック」に向く。rhino-cli が既に持つ `inspect_type`/`decompile_method`（.NET 解剖系）と相性が良い：**型を調べて → その場で C# で叩く**という一貫したワークフローが完成する。
- 1 handler に収束、薄い。

**実装方針**:
- `rhino.run_csharp` handler を追加。`source` / `result_expression`（run_python と対称）/ stdout 捕捉。
- ドキュメント追従: `docs/protocol.md`（schema）、`README.md`、`plugin/RhinoCliPlugin/README.md`、`docs/protocol.md` のレシピ集に C# 版を併記。
- CLI: `rhino-cli run-csharp`（`run-python` と対称）。

### 2. クラッシュログ解析を doctor に統合

**何**: RhinoMCP の `RhinoCrashReportFinder`（`rhino/router/RhinoCrashReportFinder.cs`）相当。Rhino のクラッシュレポートを走査し、起動失敗時に理由（ライセンスダイアログ・プラグインロード失敗等）を返す。

**なぜ**:
- ログファイル解析は run_python では不可（そもそも Rhino が起きていない場面で使う）。
- rhino-cli の `doctor` は既に「ActiveDoc 有無まで判定」する自律切り分けツール。CLAUDE.md も「失敗時は doctor で切り分け」を運用前提にしている。クラッシュ理由の自動診断はこの思想の純粋な強化。

**実装方針**:
- `doctor` に「Rhino プロセス無し or RPC 不達」のとき、直近クラッシュレポートを探して理由を併記する分岐を追加。
- RhinoMCP のログ探索パスロジックを macOS 向けに移植（Win は後回し可）。
- ドキュメント追従: `README.md`, `docs/design.md`。

### 3. 診断 Resource（host environment / installed plugins）

**何**: RhinoMCP の `HostEnvironmentResource` / `InstalledPluginsResource`（`rhino/plugin/src/Resources/`）相当の情報。Rhino バージョン・ビルド・ロード済みプラグイン一覧など。

**なぜ**:
- `capabilities`（自己記述）と `doctor`（診断）の自然な延長。AI が「いま何が使える環境か」を1コマンドで把握できる。
- 一部は run_python でも取れるが、起動診断フェーズ（plugin が確実に応答する最初の経路）で構造化して欲しい情報なので handler 化に意味がある。

**実装方針**:
- `system.version` を拡張、または `rhino.host_info` / `rhino.installed_plugins` を追加（1概念ずつ）。
- ドキュメント追従: `docs/protocol.md`, `README.md`。

### 4. 同梱 Claude Code エージェント/スキル

**何**: RhinoMCP の `cc-plugin`（エージェント8種・コマンド5種・スキル2種）に相当する、rhino-cli 利用前提の Claude Code 資産。

**なぜ**:
- rhino-cli は強力だが「CLI と capabilities を AI がどう組み立てるか」は利用者任せ。RhinoMCP は modeller/inspector/teacher 等のエージェントと snapshot/scene/launch コマンドを同梱して、初手の体験を作っている。
- これはコード本体の薄さを一切壊さない（ドキュメント/設定資産）。capabilities 駆動の自己記述と組めば「エージェントが capabilities を読んで自律操作」する形に落とせる。

**実装方針（rhino-cli 流に翻案）**:
- スキル例: `launch-and-verify`（launch → wait-ready → doctor → capabilities を定型化）、`rhino-inspect`（inspect_type/search_types/decompile_method を使った API 探索）、`run-recipe`（run_python/run_csharp レシピ集の適用）。
- コマンド例: `scene`（run_python で doc サマリ）、`snapshot`（capture_viewport）。
- これらは rhino-cli リポジトリではなく利用側（`.claude/`）に置く想定。同梱するなら配布物に含める設計を別途検討。

### 5. list_objects 相当を run_python レシピ化（handler 化はしない）

**何**: RhinoMCP の `list_objects`（型/レイヤ/可視でフィルタしたオブジェクト一覧）は AI が最頻用する。だが rhino-cli では **handler を作らない**（境界ポリシー違反）。代わりに `docs/protocol.md` の run_python レシピ集に「フィルタ付き一覧」を1行レシピとして追加する。

**なぜ**: 「便利だから handler 化」を防ぐのが境界ポリシーの肝。頻用パターンはレシピで提供するのが rhino-cli の正解。

---

## 条件付き検討（trade-off を明示してから）

### 6. Grasshopper サポート（最大ギャップ・最大リスク）

- **価値**: 現状 rhino-cli の最大の機能欠落。GH は設計用途で需要が大きい。
- **境界ポリシー的評価**: canvas 操作（コンポ配置・配線・solve）は RhinoCommon から触れない GH SDK 領域で、UI スレッド制御も伴う → 「run_python 困難」を**満たす**。ここは handler 化の正当性がある。
- **リスク**: RhinoMCP は GH1+GH2 で **23 tool**。place/connect/solve/search/describe… と**概念が分裂**し、rhino-cli の「薄い基盤」を確実に太らせる。これは境界ポリシーの第2条件（1概念に収束）と正面衝突。
- **推奨スタンス**: 入れるなら **最小核に絞る**か、**別プラグインに分離**する。
  - 最小核案: `gh.start` / `gh.run_definition`（.gh ファイルを開いて solve）/ `gh.get_canvas`（構造取得）の 3〜4 個まで。配置/配線の細粒度 tool は入れず、定義ファイル or `apply_graph` 風の宣言的1発に寄せる。
  - 分離案: `RhinoCliPlugin` 本体には入れず、`<plugin>.*` namespace の別 handler 群（例 `gh.*`）として独立配布。CLAUDE.md の「プラグイン固有 handler は `<plugin>.*` に分ける」方針に合致。
- **結論**: 即輸入はしない。やるなら上記いずれかの形で設計を固めてから。

### 7. MCP サーバ frontend（stdio）

- **価値**: RhinoMCP は MCP サーバなので Claude Desktop 等が直結できる。rhino-cli は CLI のみで、MCP クライアントからは直接使えない。`call` + `capabilities` の基盤は既にあるので、それを stdio MCP として薄くラップすれば「CLI でも MCP でも使える」になる。
- **トレードオフ**: CLI 設計と直交する追加表面。MVP の責務 MECE を優先する現方針では、まず CLI/protocol を固めてからが筋。
- **結論**: 将来候補。`capabilities` を MCP の `tools/list` に射影する形なら自然に実装できる。

---

## 輸入しない（明確に NG）

| RhinoMCP tool | 理由 | rhino-cli での代替 |
|---|---|---|
| `open_doc` / `save_doc` / `close_doc` | run_python で完結 | run_python レシピ（既存方針） |
| `get_selection` / `set_selection` | run_python で完結 | run_python レシピ |
| `set_camera` / `zoom_to_object` / `zoom_to_layer` | run_python / run_script で完結 | `capture_viewport` 引数 or レシピ |
| `set_layer_material` | run_python で完結 | run_python レシピ |
| `get_viewport_image` | **既に保有**（`capture_viewport`） | — |
| `get_commands` | **既に保有**（`list_commands`） | — |

理由は一貫して「RhinoCommon 直叩きで完結 = run_python で書ける = handler を作らない」。RhinoMCP がこれらを tool 化しているのは MCP プラットフォームとして AI に即戦力を渡す思想で、rhino-cli の薄い基盤思想とは別。**ここを真似ると rhino-cli の設計が崩れる**ので輸入しない。

---

## 推奨ロードマップ

1. **run_csharp**（小・即効・思想一致）← まずこれ
2. **doctor のクラッシュ解析統合** + **診断 Resource**（自律運用の底上げ）
3. **同梱 Claude Code スキル/コマンド**（利用体験、コードに無影響）
4. **run_python レシピ集の拡充**（list_objects 等の頻用パターン）
5. （設計確定後）**Grasshopper 最小核 or 別プラグイン**
6. （将来）**MCP サーバ frontend**

slot/router は当面スコープ外。
</content>
