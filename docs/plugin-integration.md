# Plugin Integration

This guide shows how to embed `RhinoCli.Server` into a Rhino 8 plugin.

## 1. Reference The Server Library

Add a project reference from your plugin project:

```xml
<ItemGroup>
  <ProjectReference Include="../path/to/RhinoCli.Server/RhinoCli.Server.csproj" />
</ItemGroup>
```

For Rhino 8 plugins, target `net8.0` unless your plugin has a specific multi-targeting requirement.

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
- `rpc.capabilities`
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

[HandlerMetadataAttribute(
    "Run the plugin-specific operation.",
    ParamsSchema = "{ count: number }",
    ResultSchema = "{ ok: boolean }",
    Examples = new[] { "rhino-cli call myplugin.do_thing '{\"count\":3}'" },
    SideEffects = "Depends on plugin implementation.",
    Category = "myplugin")]
public sealed class DoThingHandler : IHandler
{
    public object Execute(JsonNode? @params)
    {
        return new { ok = true };
    }
}
```

`HandlerRegistry.Register(method, handler)` reads `HandlerMetadataAttribute` from the handler class. If you need to override metadata at registration time, use `Register(method, handler, metadata)`.

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
rhino-cli doctor --port 50063
rhino-cli launch
rhino-cli wait-ready --port 50063 --timeout 120
rhino-cli ping --port 50063 --verbose
rhino-cli capabilities --port 50063
rhino-cli call myplugin.do_thing '{}' --port 50063 --pretty
rhino-cli new-model --port 50063
rhino-cli run-script "_Zoom _Extents" --port 50063
rhino-cli history --tail 50 --port 50063
rhino-cli screenshot --out /tmp/rhino-myplugin.png
rhino-cli shutdown
```

`rhino-cli launch` only starts Rhino — it has no port concept. Configure your plugin's port via your own settings/env (the example above hard-codes `50063` in the C# constructor) and use `wait-ready --port <PORT>` to block until the plugin answers `system.ping`. `rhino-cli plugin set-port <PORT>` is specific to the **bundled** RhinoCliPlugin and does not affect third-party plugins.

## Notes

- Bind only to loopback. `TcpServer` uses `IPAddress.Loopback`.
- Use a unique port per plugin.
- Keep handlers short. Long handlers block Rhino's UI thread.
- For long-running workflows, return a job id and expose a separate status method.
- Add `HandlerMetadataAttribute` to custom handler classes so `rhino-cli capabilities` can explain params, examples, and side effects to AI agents.
- `launch` and `shutdown` currently automate Rhino on macOS with `open` and AppleScript.
- `launch` by default opens a new modeling window at startup via Rhino's `-runscript _NoEcho` so `Rhino.RhinoDoc.ActiveDoc` is set immediately. Pass `--no-new-model` only if you want Rhino's start window (recent/template picker) — note `ActiveDoc` will stay `None` and plugin panel/python operations will silently fail until you dismiss it. `wait-ready` and `doctor` warn whenever `ActiveDoc` is `None` after the plugin becomes reachable. If Rhino is already running, use `--restart` to re-apply launch-time flags.
- `screenshot` captures the Rhino window on macOS and does not require any plugin RPC handler. The terminal running `rhino-cli` needs macOS Screen Recording permission.
- `new-model`, `run-script`, `history`, `list-commands`, and `probe-command` require the plugin to register `rhino.new_model`, `rhino.run_script`, `rhino.command_history`, `rhino.clear_command_history`, `rhino.list_commands`, and `rhino.probe_command`. `plugin/RhinoCliPlugin` provides the core reference handlers.

### XML doc lookup (used by `rhino.inspect_type`)

`InspectTypeHandler` enriches each member of the inspected type with
its XML documentation summary when one is available. The loader looks
for `<AssemblyName>.xml` (e.g. `RhinoCommon.xml`) **next to the
loaded assembly DLL** — the same location MSBuild emits doc files when
`GenerateDocumentationFile` is enabled. The lookup is per-assembly and
cached for the lifetime of the host process.

- For RhinoCommon, the XML file is shipped beside `RhinoCommon.dll`
  in `Rhino.app/Contents/Frameworks/RhCore.framework/.../Resources/`,
  so summaries appear out of the box on macOS Rhino 8.
- Third-party plugins that want their own types to expose summaries
  should enable `<GenerateDocumentationFile>true</GenerateDocumentationFile>`
  in their `.csproj` so the resulting `.xml` is copied alongside the
  `.rhp` / `.dll`.
- Missing XML file or unmatched member ID returns an empty string in
  the `summary` field rather than an error, so callers can always
  consume the result uniformly.
- Member IDs follow the C# documentation comment spec
  (T:/M:/P:/F:/E:), including `#ctor` for constructors and
  `\`\`N` arity markers for generic methods.

### Method body decompilation (used by `rhino.decompile_method`)

`DecompileMethodHandler` depends on the
[`ICSharpCode.Decompiler`](https://www.nuget.org/packages/ICSharpCode.Decompiler/)
NuGet package (added in `RhinoCli.Server.csproj`). One
`CSharpDecompiler` instance per assembly path is created lazily and
cached for the lifetime of the host process — first-time decompilation
of a large assembly like `RhinoCommon` allocates a few hundred MB to
build its type system. This is acceptable because plugin processes are
long-lived and the cost is paid only when a decompile request actually
fires.

Plugins that bundle their own DLLs into a different path do not need
any extra wiring; the handler resolves the assembly path from the
reflection `Type.Assembly.Location` of the requested type.
