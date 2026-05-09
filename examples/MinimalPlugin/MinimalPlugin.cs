using System.Text.Json.Nodes;
using Rhino;
using Rhino.PlugIns;
using RhinoCli.Server;

namespace MinimalPlugin;

public sealed class MinimalPlugin : PlugIn
{
    private const int Port = 50099;
    private TcpServer? _server;

    public override PlugInLoadTime LoadTime => PlugInLoadTime.AtStartup;

    protected override LoadReturnCode OnLoad(ref string errorMessage)
    {
        try
        {
            RhinoApp.CommandWindowCaptureEnabled = true;
            var registry = new HandlerRegistry("MinimalPlugin", Port);
            registry.Register("minimal.hello", new HelloHandler());
            registry.Register("minimal.echo", new EchoHandler());
            registry.Register("rhino.run_script", new RunScriptHandler());
            registry.Register("rhino.command_history", new CommandHistoryHandler());
            registry.Register("rhino.clear_command_history", new ClearCommandHistoryHandler());

            _server = new TcpServer(Port, registry, "MinimalPlugin", InvokeOnUiThread);
            _server.OnError += message => RhinoApp.WriteLine($"MinimalPlugin rhino-cli: {message}");
            _server.Start();

            var message = $"MinimalPlugin rhino-cli server listening on 127.0.0.1:{_server.ActualPort}";
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

    private sealed class EchoHandler : IHandler
    {
        public object? Execute(JsonNode? @params) => @params;
    }
}
