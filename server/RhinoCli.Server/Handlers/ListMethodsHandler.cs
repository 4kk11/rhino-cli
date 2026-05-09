using System.Text.Json.Nodes;

namespace RhinoCli.Server.Handlers;

[HandlerMetadataAttribute(
    "Return registered method names only. Use rpc.capabilities for full metadata.",
    ParamsSchema = "null",
    ResultSchema = "{ methods: string[] }",
    Examples = new[] { "rhino-cli list-methods" },
    DedicatedCommand = "rhino-cli list-methods",
    Category = "rpc")]
public sealed class ListMethodsHandler : IHandler
{
    private readonly HandlerRegistry _registry;

    public ListMethodsHandler(HandlerRegistry registry)
    {
        _registry = registry;
    }

    public object Execute(JsonNode? @params) => new
    {
        methods = _registry.Methods
    };
}
