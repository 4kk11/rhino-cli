using System.Text.Json.Nodes;
using RhinoCli.Server;

var port = ReadPort(args);
var registry = new HandlerRegistry("RhinoCli.TestRunner", port);
registry.Register("test.echo", new EchoHandler());

using var server = new TcpServer(port, registry, "RhinoCli.TestRunner");
server.OnError += message => Console.Error.WriteLine(message);
server.Start();

Console.WriteLine($"READY {server.ActualPort}");
Console.Out.Flush();

var stop = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
Console.CancelKeyPress += (_, eventArgs) =>
{
    eventArgs.Cancel = true;
    stop.TrySetResult();
};

await stop.Task;

static int ReadPort(string[] args)
{
    for (var i = 0; i < args.Length - 1; i++)
    {
        if (args[i] == "--port" && int.TryParse(args[i + 1], out var port))
        {
            return port;
        }
    }

    return 0;
}

internal sealed class EchoHandler : IHandler
{
    public object? Execute(JsonNode? @params) => @params;
}
