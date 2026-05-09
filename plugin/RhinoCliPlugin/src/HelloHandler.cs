using System.Text.Json.Nodes;
using RhinoCli.Server;

namespace RhinoCliPlugin;

public sealed class HelloHandler : IHandler
{
    public object Execute(JsonNode? @params) => new
    {
        hello = "world"
    };
}
