using System.Text.Json.Nodes;

namespace RhinoCli.Server.Handlers;

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
