# MinimalPlugin

Minimal Rhino 8 plugin example that embeds `RhinoCli.Server`.

## Build

```bash
dotnet build examples/MinimalPlugin/MinimalPlugin.csproj
```

The output is an `.rhp` under `examples/MinimalPlugin/bin/Debug/net7.0/`.
The same artifact set is copied to Rhino's macOS plugin directory:

```text
~/Library/Application Support/McNeel/Rhinoceros/8.0/MacPlugins/MinimalPlugin
```

## Manual Check

1. Launch Rhino and wait for the plugin:

```bash
rhino-cli launch --new-model --port 50099 --timeout 120
```

2. Confirm the command history contains:

```text
MinimalPlugin rhino-cli server listening on 127.0.0.1:50099
```

3. Call the plugin from the CLI:

```bash
rhino-cli ping --port 50099
rhino-cli call minimal.hello --port 50099
rhino-cli call minimal.echo '{"value":42}' --port 50099
rhino-cli new-model --port 50099
rhino-cli run-script "_Zoom _Extents" --port 50099
rhino-cli history --tail 20 --port 50099
rhino-cli screenshot --out /tmp/rhino-minimal.png
rhino-cli history --clear --port 50099
rhino-cli shutdown
```
