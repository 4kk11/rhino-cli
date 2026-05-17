using System.Text.Json;
using System.Text.Json.Nodes;
using Rhino;
using Rhino.PlugIns;
using RhinoCli.Server;

namespace RhinoCliPlugin;

public sealed class RhinoCliPlugin : PlugIn
{
    private const int DefaultPort = 50061;
    private TcpServer? _server;

    public override PlugInLoadTime LoadTime => PlugInLoadTime.AtStartup;

    protected override LoadReturnCode OnLoad(ref string errorMessage)
    {
        try
        {
            RhinoApp.CommandWindowCaptureEnabled = true;
            var port = ResolvePort();
            var registry = new HandlerRegistry("RhinoCliPlugin", port);
            registry.Register("rhino_cli.hello", new HelloHandler());
            registry.Register("rhino_cli.echo", new EchoHandler());
            registry.Register("rhino.run_script", new RunScriptHandler());
            registry.Register("rhino.run_python", new RunPythonHandler());
            registry.Register("rhino.new_model", new NewModelHandler());
            registry.Register("rhino.command_history", new CommandHistoryHandler());
            registry.Register("rhino.clear_command_history", new ClearCommandHistoryHandler());
            registry.Register("rhino.list_commands", new ListCommandsHandler());
            registry.Register("rhino.probe_command", new ProbeCommandHandler());
            registry.Register("rhino.inspect_type", new InspectTypeHandler());

            _server = new TcpServer(port, registry, "RhinoCliPlugin", InvokeOnUiThread);
            _server.OnError += message => RhinoApp.WriteLine($"RhinoCliPlugin: {message}");
            _server.Start();

            var message = $"RhinoCliPlugin server listening on 127.0.0.1:{_server.ActualPort}";
            RhinoCliHistoryBuffer.Append(message);
            RhinoApp.WriteLine(message);
            return LoadReturnCode.Success;
        }
        catch (Exception ex)
        {
            errorMessage = ex.Message;
            return LoadReturnCode.ErrorShowDialog;
        }
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

    private static int ResolvePort()
    {
        if (TryParsePort(Environment.GetEnvironmentVariable("RHINO_CLI_PORT"), out var envPort))
        {
            return envPort;
        }

        return ReadConfiguredPort() ?? DefaultPort;
    }

    private static int? ReadConfiguredPort()
    {
        var path = ConfigPath();
        if (!File.Exists(path))
        {
            return null;
        }

        try
        {
            using var document = JsonDocument.Parse(File.ReadAllText(path));
            if (!document.RootElement.TryGetProperty("port", out var value))
            {
                return null;
            }

            if (value.ValueKind == JsonValueKind.Number
                && value.TryGetInt32(out var numericPort)
                && IsValidPort(numericPort))
            {
                return numericPort;
            }

            if (value.ValueKind == JsonValueKind.String
                && TryParsePort(value.GetString(), out var stringPort))
            {
                return stringPort;
            }
        }
        catch (Exception ex)
        {
            RhinoApp.WriteLine($"RhinoCliPlugin: failed to read config {path}: {ex.Message}");
        }

        return null;
    }

    private static string ConfigPath()
    {
        return Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "rhino-cli",
            "RhinoCliPlugin",
            "config.json");
    }

    private static bool TryParsePort(string? value, out int port)
    {
        return int.TryParse(value, out port) && IsValidPort(port);
    }

    private static bool IsValidPort(int port) => port > 0 && port <= 65535;

    [HandlerMetadataAttribute(
        "Return params unchanged for JSON-RPC diagnostics.",
        ParamsSchema = "any JSON object, array, or null",
        ResultSchema = "same as params",
        Examples = new[] { "rhino-cli call rhino_cli.echo '{\"value\":42}'" },
        Category = "diagnostic")]
    private sealed class EchoHandler : IHandler
    {
        public object? Execute(JsonNode? @params) => @params;
    }
}
