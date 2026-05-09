using System.Text.Json.Nodes;

namespace RhinoCli.Server.Handlers;

[HandlerMetadataAttribute(
    "Return the plugin id and port for this RhinoCli.Server instance.",
    ParamsSchema = "null",
    ResultSchema = "{ plugins: [{ id: string, port: number }] }",
    Examples = new[] { "rhino-cli list-plugins" },
    DedicatedCommand = "rhino-cli list-plugins",
    Category = "rpc")]
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
