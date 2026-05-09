using System.Text.Json.Nodes;

namespace RhinoCli.Server.Handlers;

[HandlerMetadataAttribute(
    "Check whether the RhinoCli.Server JSON-RPC endpoint is reachable.",
    ParamsSchema = "null",
    ResultSchema = "{ pong: boolean, server: string, version: string }",
    Examples = new[] { "rhino-cli ping" },
    DedicatedCommand = "rhino-cli ping",
    Category = "system")]
public sealed class PingHandler : IHandler
{
    private readonly ServerInfo _info;

    public PingHandler(ServerInfo info)
    {
        _info = info;
    }

    public object Execute(JsonNode? @params) => new
    {
        pong = true,
        server = _info.PluginId,
        version = _info.ServerVersion
    };
}
