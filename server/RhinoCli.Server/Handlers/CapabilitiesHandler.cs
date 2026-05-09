using System.Text.Json.Nodes;

namespace RhinoCli.Server.Handlers;

[HandlerMetadataAttribute(
    "Return handler metadata for this plugin. Pass { method } to inspect one handler.",
    ParamsSchema = "null | { method?: string }",
    ResultSchema = "{ server: object, methods?: HandlerDescriptor[], method?: HandlerDescriptor }",
    Examples = new[]
    {
        "rhino-cli capabilities",
        "rhino-cli capabilities --method rhino.run_script"
    },
    DedicatedCommand = "rhino-cli capabilities",
    Category = "rpc")]
public sealed class CapabilitiesHandler : IHandler
{
    private readonly HandlerRegistry _registry;

    public CapabilitiesHandler(HandlerRegistry registry)
    {
        _registry = registry;
    }

    public object Execute(JsonNode? @params)
    {
        var method = @params?["method"]?.GetValue<string>();
        var server = new
        {
            plugin_id = _registry.Info.PluginId,
            port = _registry.Info.Port,
            server_version = _registry.Info.ServerVersion
        };

        if (string.IsNullOrWhiteSpace(method))
        {
            return new
            {
                server,
                methods = _registry.Capabilities
            };
        }

        if (!_registry.TryGetCapability(method, out var capability))
        {
            throw new RpcException(-32601, "Method not found", new { method });
        }

        return new
        {
            server,
            method = capability
        };
    }
}
