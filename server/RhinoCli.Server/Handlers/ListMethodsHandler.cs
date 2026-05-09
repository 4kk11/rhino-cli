using System.Text.Json.Nodes;

namespace RhinoCli.Server.Handlers;

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
