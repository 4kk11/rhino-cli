# RhinoCliPlugin

Core Rhino 8 plugin for `rhino-cli`. It embeds `RhinoCli.Server` and registers the standard Rhino automation handlers used by the CLI.

## Build

```bash
dotnet build plugin/RhinoCliPlugin/RhinoCliPlugin.csproj
```

The output is an `.rhp` under `plugin/RhinoCliPlugin/bin/Debug/net7.0/`.
The same artifact set is copied to Rhino's macOS plugin directory:

```text
~/Library/Application Support/McNeel/Rhinoceros/8.0/MacPlugins/RhinoCliPlugin
```

## Yak Package

Publishing this plugin to the [Yak server](https://yak.rhino3d.com) is automated through `cargo make`:

```bash
cargo make yak-build   # Release build + manifest copy + `yak build --platform mac`
cargo make yak-push    # uploads the most recent .yak (requires `yak login` once)
```

`manifest.yml` in this directory drives the package metadata (`name`, `version`, `authors`, `description`, `url`, `keywords`). Bump `version` here together with `AssemblyVersion` / `FileVersion` in `RhinoCliPlugin.csproj` before publishing.

## Manual Check

1. Launch Rhino and wait for the plugin:

```bash
rhino-cli plugin set-port 50061
rhino-cli launch --new-model
rhino-cli wait-ready --port 50061 --timeout 120
```

2. Confirm the command history contains:

```text
RhinoCliPlugin server listening on 127.0.0.1:50061
```

3. Call the plugin from the CLI:

```bash
rhino-cli doctor --port 50061
rhino-cli ping --port 50061
rhino-cli capabilities --format agent --port 50061
rhino-cli call rhino_cli.hello --port 50061
rhino-cli call rhino_cli.echo '{"value":42}' --port 50061
rhino-cli new-model --port 50061
rhino-cli list-commands --pattern Box --port 50061
rhino-cli probe-command Box --port 50061
rhino-cli inspect-type Rhino.Geometry.Box --port 50061
rhino-cli search-types AddBox --port 50061
rhino-cli decompile-method Rhino.Geometry.Box ClosestPoint --signature Point3d --port 50061
rhino-cli run-script "_Zoom _Extents" --port 50061
rhino-cli history --tail 20 --port 50061
rhino-cli screenshot --out /tmp/rhino-cli-plugin.png
rhino-cli capture-viewport --width 800 --height 600 --mode Shaded --out /tmp/rhino-cli-viewport.png
rhino-cli history --clear --port 50061
rhino-cli shutdown
```

## Registered Handlers

The plugin exposes a deliberately small surface. Anything that can be expressed as a few lines of RhinoCommon Python goes through `rhino.run_python` instead of getting its own bespoke handler. See `CLAUDE.md` ("ハンドラ追加の境界ポリシー") and `docs/protocol.md` for the policy and recipes.

| Method | Why it exists |
|--------|---------------|
| `rhino.new_model` | Create a new active document. |
| `rhino.run_script` | Run a Rhino command script. Returns `objects_added` / `objects_removed` / `command_prompt_changed` / `history_delta` so the caller can tell whether the underlying command actually did anything. |
| `rhino.run_python` | Execute an inline Python source string. The official escape hatch for arbitrary RhinoCommon work; pass `result_expression` to evaluate a final expression and receive its value JSON-serialized. |
| `rhino.command_history` / `rhino.clear_command_history` | Read or clear Rhino's command history (reflects the Eto CommandHistoryViewModel via reflection — not feasible from `run_python` alone). |
| `rhino.list_commands` | Enumerate Rhino command names known to the running instance. |
| `rhino.probe_command` | Start `! _-{Name} _Cancel × 5` and capture the first Get prompt + Write/WriteLine output. Uses background-thread `RhinoApp.SendKeystrokes("")` for forced cancel; not reproducible from `run_python`. |
| `rhino.inspect_type` | Reflect on a .NET type loaded in the Rhino process and return its constructors, properties, methods (overload-grouped), events, and fields as structured JSON. AI agents use this to discover RhinoCommon API signatures before writing `run_python`. FQN-only resolution; pair with `search_types` to look up FQNs from short names. Attaches XML doc `<summary>` text when an adjacent `.xml` file exists. |
| `rhino.search_types` | Walk loaded assemblies (default: `Rhino*` / `RhinoCommon` / `RhinoCli*`) and return types and members whose name contains the given substring (case-insensitive). Used in front of `inspect_type` when only a short name is known. |
| `rhino.decompile_method` | Decompile one method's IL into C# via ICSharpCode.Decompiler. Pass `signature` (comma-separated parameter type names) to disambiguate overloads. Use `.ctor` for constructors. |
| `rhino.capture_viewport` | Capture a single viewport to PNG with structured display-mode / projection / camera control via `RhinoView.CaptureToBitmap`. Returns base64 + applied state. DisplayMode is non-destructive (capture overload accepts the mode); camera / projection / zoom_extents mutate the view and are not restored. `transparent_background=true` uses `DisplayPipelineAttributes.FillMode = Transparent`. |

For workflows previously covered by `save_document`, `open_document`, `list_objects`, `delete_objects`, `add_box`, and `add_box_3point`, see the recipe block under `rhino.run_python` in `docs/protocol.md`.
