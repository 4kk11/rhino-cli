# rhino-cli

Rust 製の汎用 Rhino プラグイン用 RPC クライアント + C# サーバライブラリ。任意の Rhino プラグインに JSON-RPC 2.0 over TCP インタフェースを追加し、CLI から呼び出して E2E テスト・自動化・スクリプティングを実現する。

> **Status**: Pre-alpha MVP. Rust CLI, C# server library, mock E2E runner, and RhinoCliPlugin are implemented.

## 構成

| 場所 | 内容 |
|------|------|
| `src/` | Rust 製 CLI バイナリ (`rhino-cli`) |
| `server/RhinoCli.Server/` | C# クラスライブラリ (TCP server + Router + 組込 handler) |
| `plugin/RhinoCliPlugin/` | rhino-cli 同梱の Rhino automation プラグイン |
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

Recommended task runner:

```bash
cargo install cargo-make
cargo make check
cargo make install-cli
```

Individual tasks:

```bash
cargo make test
cargo make build
cargo make install-cli
```

Equivalent raw commands:

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
rhino-cli doctor --port 50061
rhino-cli plugin set-port 50061
rhino-cli launch --new-model
rhino-cli wait-ready --port 50061 --timeout 120
rhino-cli ping --port 50061 --verbose
rhino-cli capabilities --port 50061
rhino-cli capabilities --method rhino.run_script --port 50061
rhino-cli list-plugins --port 50061
rhino-cli call system.version --port 50061 --pretty
rhino-cli new-model --port 50061
rhino-cli list-commands --pattern Box --port 50061
rhino-cli probe-command Box --port 50061
rhino-cli inspect-type Rhino.Geometry.Box --port 50061
rhino-cli search-types AddBox --port 50061
rhino-cli run-script "_Zoom _Extents" --port 50061
rhino-cli history --tail 50 --port 50061
rhino-cli call rhino.run_python '{"source":"import scriptcontext as sc; print(sc.doc.Objects.Count)"}' --port 50061
rhino-cli call rhino.run_python '{"source":"import scriptcontext as sc, Rhino.FileIO as fio\nopts=fio.FileWriteOptions(); opts.FileVersion=8; opts.SuppressDialogBoxes=True\nok=sc.doc.WriteFile(\"/tmp/test.3dm\", opts)","result_expression":"ok"}' --port 50061
rhino-cli screenshot --out /tmp/rhino-window.png
rhino-cli shutdown
```

`launch` and `shutdown` currently automate Rhino on macOS via the installed app name. The default app is `Rhino 8`; use `--app "RhinoWIP"` or `--app "Rhino 7"` when needed. `launch` only starts Rhino — it does not configure plugin ports and does not wait for the RPC endpoint. Use `rhino-cli wait-ready --port <PORT>` after launch when you need to block until the plugin answers `system.ping`. `launch --restart` asks Rhino to quit before relaunching. `launch --new-model` opens a modeling window at startup instead of leaving Rhino's start window active. `launch --script "<Rhino command script>"` passes a Rhino `-runscript` argument.

`plugin set-port <PORT>` writes the bundled RhinoCliPlugin's launch config (`~/Library/Application Support/rhino-cli/RhinoCliPlugin/config.json` on macOS). The plugin reads it the next time Rhino loads, so call this before `launch` (or before restarting Rhino) when you need to change the listening port. `plugin show-config` prints the current contents. Third-party plugins that embed `RhinoCli.Server` configure their own ports independently; `plugin set-port` only affects the bundled plugin.

`doctor` answers whether Rhino and the RhinoCliPlugin RPC endpoint are reachable. `capabilities` is the self-describing command for AI agents and humans: it prints registered handlers, params, result shapes, examples, side effects, and dedicated CLI wrappers. Use `--format json`, `--format markdown`, or `--format agent` when another tool needs structured context.

`call` is the universal execution path for any registered handler.

`list-plugins` calls `rpc.list_plugins` and prints `<plugin id>\t<port>` per line. Use `--raw --pretty` to dump the JSON shape directly.

`run-script` prints Rhino's `RunScript` result JSON. Use `--fail-on-false` when a false return value should fail automation.

`new-model` calls the plugin's `rhino.new_model` handler. It creates another unsaved model from Rhino's default template, or from `--template <3dm>` when specified.

`list-commands` returns Rhino command names known to the running instance (English by default). Use `--pattern` for a case-insensitive substring filter and `--include-unloaded` to also include commands from unloaded plugins. `probe-command <NAME>` starts the command via `! _-<Name> _Cancel` and returns the captured first prompt and option labels (in Rhino's locale) so AI agents can discover argument syntax dynamically before invoking `run-script`. The option short codes in parentheses (e.g. `(D)`, `(P)`) are ASCII-stable across locales and can be passed directly as `_D`, `_P`. Use with care for commands that have immediate side effects or no `-` (no-dialog) variant.

`inspect-type <FQN>` reflects on a .NET type loaded in the Rhino process and returns its constructors, properties, methods (grouped by overload), events, and fields as structured JSON. Use this before writing `run_python` against an unfamiliar RhinoCommon type — it surfaces exact parameter types and overload shapes that `dir()` cannot expose. Resolution is fully qualified name only (no suffix matching); pair with `search-types` to look up an FQN from a short name. Use `--binding public_static` (or `public_instance`, `non_public`, `all`) to change visibility, and `--include-inherited` to include base members. XML documentation summaries are attached automatically when an `<AssemblyName>.xml` file (e.g. `RhinoCommon.xml`) is found next to the loaded DLL.

`search-types <PATTERN>` finds types and members whose name contains `PATTERN` (case-insensitive) across the loaded assemblies. By default the search is restricted to `Rhino*`, `RhinoCommon`, and `RhinoCli*`; use `--assembly <NAME>` to target a specific assembly, `--scope types` or `--scope members` to narrow the kind, and `--limit N` for the result cap (default 50, `truncated:true` is set when more matches were skipped). Typical workflow: run `search-types AddBox` to find `Rhino.DocObjects.Tables.ObjectTable.AddBox`, then call `inspect-type` against the discovered type to read its overloads.

`screenshot` captures the Rhino app window itself as a PNG on macOS. It is useful for autonomous visual debugging after `run-script`; use `--no-shadow` for tighter images, `--no-activate` when you have already focused Rhino, and `--window-id` for a known macOS window id. macOS Screen Recording permission is required for the terminal process running `rhino-cli`. For pixel-only viewport rendering call `RhinoView.CaptureToBitmap` via `rhino.run_python` (recipe in `docs/protocol.md`).

### Handler set is intentionally small

The bundled plugin exposes a minimal handler surface: lifecycle / introspection only. Anything that can be expressed in a few lines of RhinoCommon Python — saving / opening files, adding geometry, listing or deleting objects by ID, capturing viewports, batch operations — goes through `rhino.run_python` with `result_expression` rather than getting a dedicated handler. This keeps the protocol from accumulating `add_box → add_sphere → add_cylinder → ...` style bloat.

Currently registered:

- `rhino.run_script` — run Rhino command scripts. Result now includes `objects_added` / `objects_removed` / `command_prompt_changed` / `history_delta`.
- `rhino.run_python` — execute inline Python with `scriptcontext.doc` pre-wired. Pass `result_expression` to receive a JSON-serialized return value alongside captured `stdout`.
- `rhino.new_model` — create a new active document.
- `rhino.command_history` / `rhino.clear_command_history` — read or clear Rhino's command history (uses reflection against the Eto CommandHistoryViewModel; not feasible from `run_python`).
- `rhino.list_commands` / `rhino.probe_command` — command discovery and dynamic prompt probing (probe needs background-thread `RhinoApp.SendKeystrokes("")` cancel).
- `rhino.inspect_type` — API discovery via `System.Reflection`. Returns the constructors, properties, methods (overload-grouped), events, and fields of a .NET type loaded in the Rhino process so AI agents can verify RhinoCommon signatures before writing `run_python`. FQN-only resolution. XML documentation summaries are attached when an adjacent `.xml` file exists.
- `rhino.search_types` — Find types or members by substring across loaded assemblies (case-insensitive). Pair with `inspect_type` when only a short name like `AddBox` is known.

The boundary policy and recipe collection (save / open / add box / list / delete / capture-viewport in pure `run_python`) live in `CLAUDE.md` and `docs/protocol.md`.

## RhinoCliPlugin

Build the Rhino plugin:

```bash
dotnet build plugin/RhinoCliPlugin/RhinoCliPlugin.csproj
```

The build copies the plugin artifacts to:

```text
~/Library/Application Support/McNeel/Rhinoceros/8.0/MacPlugins/RhinoCliPlugin
```

Launch Rhino 8, then call:

```bash
rhino-cli plugin set-port 50061
rhino-cli launch --new-model
rhino-cli wait-ready --port 50061 --timeout 120
rhino-cli capabilities --format agent --port 50061
rhino-cli call rhino_cli.hello --port 50061
rhino-cli new-model --port 50061
rhino-cli list-commands --pattern Box --port 50061
rhino-cli probe-command Box --port 50061
rhino-cli inspect-type Rhino.Geometry.Box --port 50061
rhino-cli search-types AddBox --port 50061
rhino-cli run-script "_Zoom _Extents" --port 50061
rhino-cli history --tail 20 --port 50061
rhino-cli screenshot --out /tmp/rhino-cli-plugin.png
rhino-cli shutdown
```

## Docs

- `docs/design.md` - architecture and scope
- `docs/protocol.md` - JSON-RPC 2.0 over TCP protocol
- `docs/plugin-integration.md` - embedding guide for existing plugins
- `docs/tasks.md` - implementation checklist

## ライセンス

MIT
