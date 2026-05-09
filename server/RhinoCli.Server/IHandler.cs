using System.Text.Json.Nodes;

namespace RhinoCli.Server;

public interface IHandler
{
    object? Execute(JsonNode? @params);
}
