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
rhino-cli run-script "_Zoom _Extents" --port 50061
rhino-cli history --tail 20 --port 50061
rhino-cli screenshot --out /tmp/rhino-cli-plugin.png
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
| `rhino.inspect_type` | Reflect on a .NET type loaded in the Rhino process and return its constructors, properties, methods (overload-grouped), events, and fields as structured JSON. AI agents use this to discover RhinoCommon API signatures before writing `run_python`. FQN-only resolution; pair with the upcoming `search_types` to look up FQNs from short names. |

For workflows previously covered by `save_document`, `open_document`, `list_objects`, `delete_objects`, `add_box`, `add_box_3point`, and `capture_viewport`, see the recipe block under `rhino.run_python` in `docs/protocol.md`.
