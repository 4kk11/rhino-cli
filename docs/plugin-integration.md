# Plugin Integration

This guide shows how to embed `RhinoCli.Server` into a Rhino 8 plugin.

## 1. Reference The Server Library

Add a project reference from your plugin project:

```xml
<ItemGroup>
  <ProjectReference Include="../path/to/RhinoCli.Server/RhinoCli.Server.csproj" />
</ItemGroup>
```

For Rhino 8 plugins, target `net7.0` unless your plugin has a specific multi-targeting requirement.

## 2. Copy Artifacts After Build

For fast local iteration, copy the built `.rhp` and dependent assemblies into Rhino's plugin folder after each build:

```xml
<PropertyGroup Condition="'$(OS)' == 'Windows_NT'">
  <MyDestinationFolder>$(APPDATA)\McNeel\Rhinoceros\myplugins\MyPlugin</MyDestinationFolder>
</PropertyGroup>

<PropertyGroup Condition="'$(OS)' != 'Windows_NT'">
  <MyDestinationFolder>$(HOME)/Library/Application Support/McNeel/Rhinoceros/8.0/MacPlugins/MyPlugin</MyDestinationFolder>
</PropertyGroup>

<Target Name="PostBuild" AfterTargets="PostBuildEvent">
  <ItemGroup>
    <PluginFiles Include="$(TargetDir)*.*" />
  </ItemGroup>
  <MakeDir Directories="$(MyDestinationFolder)" />
  <Copy SourceFiles="@(PluginFiles)" DestinationFolder="$(MyDestinationFolder)" />
  <Message Text="MyPlugin installed to $(MyDestinationFolder)" Importance="high" />
</Target>
```

## 3. Register Handlers

Create a `HandlerRegistry` in your plugin `OnLoad` method. The registry auto-registers these standard methods:

- `system.ping`
- `system.version`
- `rpc.list_methods`
- `rpc.list_plugins`

Register plugin-specific methods under your own namespace:

```csharp
var registry = new HandlerRegistry("MyPlugin", 50063);
registry.Register("myplugin.do_thing", new DoThingHandler());
```

Handlers implement `IHandler`:

```csharp
using System.Text.Json.Nodes;
using RhinoCli.Server;

public sealed class DoThingHandler : IHandler
{
    public object Execute(JsonNode? @params)
    {
        return new { ok = true };
    }
}
```

Throw `RpcException` for expected RPC failures:

```csharp
throw new RpcException(
    -32602,
    "Invalid params",
    new { field = "count", expected = "number" });
```

## 4. Start TcpServer

Rhino document and UI APIs must run on Rhino's UI thread. Pass an invoke delegate to `TcpServer` that wraps handler execution with `RhinoApp.InvokeOnUiThread`.

```csharp
using Rhino;
using Rhino.PlugIns;
using RhinoCli.Server;

public sealed class MyPlugin : PlugIn
{
    private TcpServer? _server;

    public override PlugInLoadTime LoadTime => PlugInLoadTime.AtStartup;

    protected override LoadReturnCode OnLoad(ref string errorMessage)
    {
        var registry = new HandlerRegistry("MyPlugin", 50063);
        registry.Register("myplugin.do_thing", new DoThingHandler());

        _server = new TcpServer(50063, registry, "MyPlugin", InvokeOnUiThread);
        _server.OnError += message => RhinoApp.WriteLine($"rhino-cli: {message}");
        _server.Start();
        return LoadReturnCode.Success;
    }

    protected override void OnShutdown()
    {
        _server?.Stop();
        _server = null;
    }

    private static object? InvokeOnUiThread(IHandler handler, JsonNode? @params)
    {
        object? result = null;
        Exception? error = null;

        RhinoApp.InvokeOnUiThread(new Action(() =>
        {
            try
            {
                result = handler.Execute(@params);
            }
            catch (Exception ex)
            {
                error = ex;
            }
        }));

        if (error is not null)
        {
            throw error;
        }

        return result;
    }
}
```

## 5. Verify From CLI

```bash
rhino-cli launch --new-model --port 50063 --timeout 120
rhino-cli wait-ready --port 50063 --timeout 30
rhino-cli ping --port 50063 --verbose
rhino-cli list-methods --port 50063
rhino-cli call myplugin.do_thing '{}' --port 50063 --pretty
rhino-cli new-model --port 50063
rhino-cli run-script "_Zoom _Extents" --port 50063
rhino-cli history --tail 50 --port 50063
rhino-cli screenshot --out /tmp/rhino-myplugin.png
rhino-cli shutdown
```

## Notes

- Bind only to loopback. `TcpServer` uses `IPAddress.Loopback`.
- Use a unique port per plugin.
- Keep handlers short. Long handlers block Rhino's UI thread.
- For long-running workflows, return a job id and expose a separate status method.
- `launch` and `shutdown` currently automate Rhino on macOS with `open` and AppleScript.
- `launch --new-model` opens a modeling window at startup via Rhino's launch-time `-runscript` path. If Rhino is already running, use `--restart` to apply it.
- `screenshot` captures the Rhino window on macOS and does not require any plugin RPC handler. The terminal running `rhino-cli` needs macOS Screen Recording permission.
- `new-model`, `run-script`, and `history` require the plugin to register `rhino.new_model`, `rhino.run_script`, `rhino.command_history`, and `rhino.clear_command_history`. `examples/MinimalPlugin` provides reference handlers.
