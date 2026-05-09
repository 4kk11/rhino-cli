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
rhino-cli run-script "_Zoom _Extents" --port 50061
rhino-cli history --tail 20 --port 50061
rhino-cli screenshot --out /tmp/rhino-cli-plugin.png
rhino-cli history --clear --port 50061
rhino-cli shutdown
```
