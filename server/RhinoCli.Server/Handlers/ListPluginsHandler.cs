using System.Text.Json.Nodes;

namespace RhinoCli.Server.Handlers;

public sealed class ListPluginsHandler : IHandler
{
    private readonly ServerInfo _info;

    public ListPluginsHandler(ServerInfo info)
    {
        _info = info;
    }

    public object Execute(JsonNode? @params) => new
    {
        plugins = new[]
        {
            new
            {
                id = _info.PluginId,
                port = _info.Port
            }
        }
    };
}
