using System.Text.Json.Nodes;

namespace RhinoCli.Server.Handlers;

public sealed class VersionHandler : IHandler
{
    private readonly ServerInfo _info;

    public VersionHandler(ServerInfo info)
    {
        _info = info;
    }

    public object Execute(JsonNode? @params) => new
    {
        protocol = "jsonrpc-2.0",
        server = _info.ServerVersion,
        plugin = _info.PluginId
    };
}
