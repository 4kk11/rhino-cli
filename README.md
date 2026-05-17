# rhino-cli

A generic JSON-RPC 2.0 client (Rust CLI) and server library (C#) for Rhino plugins. Drop the server library into any Rhino plugin to expose its operations over TCP, then drive them from the CLI for end-to-end testing, automation, and scripting. The bundled `RhinoCliPlugin` provides the standard Rhino automation handlers used by AI agents and humans alike.

> Status: Pre-alpha MVP. The Rust CLI, C# server library, mock E2E runner, and `RhinoCliPlugin` are all implemented.

日本語版: [README.ja.md](README.ja.md)

## Architecture

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

Each Rhino plugin embeds `RhinoCli.Server` as a NuGet dependency and registers its own handlers. `rhino-cli` connects to whichever plugin port is running inside the live Rhino process and dispatches RPC calls.

## Repository Layout

| Path | Purpose |
|------|---------|
| `src/` | Rust CLI binary (`rhino-cli`) |
| `server/RhinoCli.Server/` | C# class library: TCP server, message router, built-in handlers |
| `server/RhinoCli.Server.Tests/` | Server unit tests |
| `server/RhinoCli.TestRunner/` | Mock end-to-end runner |
| `plugin/RhinoCliPlugin/` | Bundled Rhino automation plugin |
| `docs/` | Design, protocol, integration, task list |

## Use Cases

- End-to-end regression testing across multiple Rhino plugins (driven autonomously by AI agents)
- Batch automation and CI integration
- Live state inspection and debugging of a running Rhino instance

## Requirements

- Rust 1.75+ (to build the CLI)
- .NET 7.0 SDK (to build the server library and plugin)
- Rhino 8 (host application)

## Install

The CLI is published to [crates.io](https://crates.io/crates/rhino-cli) and the bundled Rhino plugin to the [Yak server](https://yak.rhino3d.com). Install both:

```bash
# CLI
cargo install rhino-cli

# RhinoCliPlugin (run from Rhino 8 command line, or use Rhino's Package Manager UI)
_PackageManager
# Search for: rhino-cli
```

Equivalent Yak CLI invocation:

```bash
"/Applications/Rhino 8.app/Contents/Resources/bin/yak" install rhino-cli
```

## Build from Source

Recommended task runner (cargo-make):

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
cargo make build-plugin
```

Equivalent raw commands:

```bash
cargo build
cargo test -- --test-threads=1
dotnet test server/RhinoCli.Server.Tests/RhinoCli.Server.Tests.csproj
dotnet build server/RhinoCli.TestRunner/RhinoCli.TestRunner.csproj
cargo install --path .
```

## Running Against a Plugin Server

A typical session against the bundled plugin:

```bash
rhino-cli plugin set-port 50061
rhino-cli launch --new-model
rhino-cli wait-ready --port 50061 --timeout 120
rhino-cli doctor --port 50061
rhino-cli capabilities --format agent --port 50061
rhino-cli call system.version --port 50061 --pretty
rhino-cli new-model --port 50061
rhino-cli run-script "_Zoom _Extents" --port 50061
rhino-cli capture-viewport --width 1280 --height 720 --out /tmp/viewport.png --port 50061
rhino-cli shutdown
```

The responsibilities are intentionally split: `launch` only starts the Rhino process, `plugin set-port` configures the bundled plugin's listening port, and `wait-ready` blocks until the RPC endpoint answers `system.ping`. Combine them rather than expecting any single command to do all three.

## CLI Subcommand Reference

| Command | Key flags | Description |
|---------|-----------|-------------|
| `ping` | — | Call `system.ping`. |
| `doctor` | `--app` | Diagnose Rhino + plugin reachability. |
| `capabilities` | `--method`, `--format {text,json,markdown,agent}` | Print the self-describing handler catalog. |
| `call` | `<method> [params_json]`, `--params-file`, `--param k=v` | Universal RPC entry point. |
| `list-methods` | — | List registered RPC method names. |
| `list-plugins` | — | List reachable `RhinoCli.Server` plugin instances (`<id>\t<port>`). |
| `wait-ready` | (uses `--timeout`) | Block until `system.ping` succeeds. |
| `launch` | `--app`, `--restart`, `--new-model`, `--script` | Start Rhino (macOS). |
| `shutdown` | `--app` | Ask Rhino to quit and wait for exit (macOS). |
| `plugin set-port` | `<port>` | Write the bundled `RhinoCliPlugin` launch config. |
| `plugin show-config` | — | Print the current bundled plugin launch config. |
| `screenshot` | `--out`, `--no-shadow`, `--no-activate`, `--window-id` | macOS-level window capture via `screencapture`. |
| `capture-viewport` | `--width`, `--height` (required), `--viewport`, `--mode`, `--projection`, `--camera`, `--target`, `--zoom-extents`, `--transparent`, `--out` | In-process viewport capture (PNG, base64). |
| `new-model` | `--template <3dm>` | Create a new active document. |
| `run-script` | `<script>`, `--echo`, `--mru`, `--fail-on-false` | Execute a Rhino command script. |
| `history` | `--tail`, `--clear`, `--json` | Read or clear Rhino command history. |
| `list-commands` | `--pattern`, `--include-unloaded` | List Rhino command names. |
| `probe-command` | `<name>` | Start a command and capture its first prompt + option labels. |
| `inspect-type` | `<fqn>`, `--binding`, `--include-inherited`, `--with-body` | Reflect a .NET type loaded inside Rhino. |
| `search-types` | `<pattern>`, `--scope`, `--assembly`, `--limit` | Find types or members by case-insensitive substring. |
| `decompile-method` | `<type> <method>`, `--signature` | Decompile a single .NET method to C#. |

Global flags (apply to every subcommand):

- `--port` (env `RHINO_CLI_PORT`, default `50061`)
- `--host` (env `RHINO_CLI_HOST`, default `localhost`)
- `--timeout`, `--connect-timeout`
- `--pretty`, `--raw`
- `-q/--quiet`, `-v/--verbose`

### Notes on selected subcommands

- **`launch`** only starts Rhino. It does not write plugin configuration and does not wait for the RPC endpoint. Pair it with `plugin set-port` (before) and `wait-ready` (after). `--restart` quits Rhino first; `--new-model` opens a modeling window at startup; `--script` is passed through as Rhino's `-runscript` argument.
- **`plugin set-port`** writes `~/Library/Application Support/rhino-cli/RhinoCliPlugin/config.json` on macOS. The bundled plugin reads it the next time Rhino loads, so call it before `launch` (or before restarting Rhino). Third-party plugins that embed `RhinoCli.Server` configure their own ports independently.
- **`capabilities`** is the self-describing API for AI agents and humans. It prints registered handlers, parameter shapes, result shapes, examples, side effects, and dedicated CLI wrappers. Use `--format json|markdown|agent` when another tool needs structured context.
- **`call`** is the universal execution path for any registered handler.
- **`screenshot`** captures the Rhino app window via `screencapture` (macOS only) and works without the plugin. The terminal process needs Screen Recording permission. Use `--no-shadow` for tighter images, `--no-activate` when Rhino is already focused, and `--window-id` for a known macOS window id.
- **`capture-viewport`** captures a single viewport in-process via the plugin's `RhinoView.CaptureToBitmap`. Display mode (`--mode`) is applied non-destructively; camera, projection, and zoom mutate the view and are not restored. `--camera`/`--target` can be combined with `--zoom-extents`. Omit `--out` to receive the full JSON-RPC result (including base64 PNG) on stdout.
- **`inspect-type`** resolves by fully qualified name only — pair it with `search-types` to look up the FQN from a short name. Use `--binding public_static|public_instance|non_public|all` to change visibility and `--include-inherited` to walk base members. XML documentation summaries are attached automatically when an `<AssemblyName>.xml` file (for example `RhinoCommon.xml`) sits next to the loaded DLL.
- **`decompile-method`** uses ICSharpCode.Decompiler. Use `.ctor` for constructors. When the method is overloaded, pass `--signature` as a comma-separated list of parameter type names (`Point3d` or `Rhino.Geometry.Point3d,bool`). To fetch shape and method body in one call, use `inspect-type --with-body <method>`.
- **`run-script`** prints Rhino's `RunScript` result JSON. Use `--fail-on-false` when a `false` return value should fail automation. The response includes `objects_added`, `objects_removed`, `command_prompt_changed`, and `history_delta`.
- **`probe-command`** starts the command via `! _-<Name> _Cancel` and returns the captured first prompt and option labels (in Rhino's current locale). The ASCII short codes in parentheses (for example `(D)`, `(P)`) are locale-stable and can be sent back as `_D`, `_P`. Use with care for commands with immediate side effects or no `-` (no-dialog) variant.

## Handlers

### Built-in handlers (RhinoCli.Server)

Auto-registered in every `HandlerRegistry`:

| Method | Description |
|--------|-------------|
| `system.ping` | Liveness check. |
| `system.version` | Server / plugin version info. |
| `rpc.capabilities` | Handler metadata catalog. |
| `rpc.list_methods` | All registered method names. |
| `rpc.list_plugins` | Plugin instances reachable on this connection. |

### Bundled handlers (RhinoCliPlugin)

Registered by the bundled plugin at startup:

| Method | Description |
|--------|-------------|
| `rhino_cli.hello` | Smoke test (returns `{hello:"world"}`). |
| `rhino_cli.echo` | Echoes its parameters unchanged. |
| `rhino.run_script` | Execute a Rhino command script. The result includes `objects_added` / `objects_removed` / `command_prompt_changed` / `history_delta`. |
| `rhino.run_python` | Execute inline Python with `scriptcontext.doc` pre-wired. `result_expression` returns a JSON-serialized value alongside captured `stdout`. |
| `rhino.new_model` | Create a new active document (optionally from a template). |
| `rhino.command_history` / `rhino.clear_command_history` | Read or clear Rhino's command history via reflection against the Eto `CommandHistoryViewModel` (not feasible from `run_python`). |
| `rhino.list_commands` / `rhino.probe_command` | Command discovery and dynamic prompt probing. `probe_command` needs a background-thread `RhinoApp.SendKeystrokes("")` cancel. |
| `rhino.inspect_type` | API discovery via `System.Reflection`. Returns constructors, properties, methods (overload-grouped), events, and fields of a .NET type loaded in the Rhino process. |
| `rhino.search_types` | Find types or members by case-insensitive substring across loaded assemblies. |
| `rhino.decompile_method` | Decompile a single method to C# via ICSharpCode.Decompiler. |
| `rhino.capture_viewport` | Capture a viewport to PNG with structured display-mode / camera / projection control. Returns `png_base64` plus applied state. |

### Why the handler set is intentionally small

The bundled plugin exposes a minimal handler surface — lifecycle and introspection only. Anything that can be expressed in a few lines of RhinoCommon Python (saving / opening files, adding geometry, listing or deleting objects by ID, batch operations) goes through `rhino.run_python` with `result_expression` rather than getting a dedicated handler. This keeps the protocol from accumulating `add_box → add_sphere → add_cylinder → …` style bloat.

The boundary policy and the `run_python` recipe collection (save / open / add / list / delete / capture) live in `CLAUDE.md` and the `rhino.run_python` section of `docs/protocol.md`.

## RhinoCliPlugin

Build the bundled Rhino plugin:

```bash
dotnet build plugin/RhinoCliPlugin/RhinoCliPlugin.csproj
```

The build copies the plugin artifacts to:

```text
~/Library/Application Support/McNeel/Rhinoceros/8.0/MacPlugins/RhinoCliPlugin
```

Smoke-test sequence:

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
rhino-cli decompile-method Rhino.Geometry.Box ClosestPoint --signature Point3d --port 50061
rhino-cli run-script "_Zoom _Extents" --port 50061
rhino-cli history --tail 20 --port 50061
rhino-cli screenshot --out /tmp/rhino-cli-plugin.png
rhino-cli shutdown
```

## Release

Two registries, two flows.

### CLI → crates.io

1. Bump `version` in `Cargo.toml`.
2. `cargo make publish-dry-run` (verifies metadata and package contents).
3. `cargo publish` (requires `cargo login`).

### RhinoCliPlugin → Yak

1. Bump `version` in `plugin/RhinoCliPlugin/manifest.yml` (and `AssemblyVersion` / `FileVersion` in `RhinoCliPlugin.csproj` to match).
2. `cargo make yak-build` — runs a Release `dotnet build`, copies the manifest into the output dir, and produces `*.yak` under `plugin/RhinoCliPlugin/bin/Release/net7.0/`.
3. `cargo make yak-push` — uploads the most recent `.yak` package (requires `yak login` once).

`YAK_BIN` env var overrides the Yak binary location (default `/Applications/Rhino 8.app/Contents/Resources/bin/yak`).

## Documentation

- `docs/design.md` — architecture and scope
- `docs/protocol.md` — JSON-RPC 2.0 over TCP protocol
- `docs/plugin-integration.md` — embedding guide for third-party plugins
- `docs/tasks.md` — implementation checklist

## License

MIT
