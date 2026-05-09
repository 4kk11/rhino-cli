using System.Text.Json.Nodes;
using RhinoCli.Server;

namespace RhinoCliPlugin;

[HandlerMetadataAttribute(
    "Return a fixed hello response for smoke testing the plugin.",
    ParamsSchema = "null",
    ResultSchema = "{ hello: string }",
    Examples = new[] { "rhino-cli call rhino_cli.hello" },
    Category = "diagnostic")]
public sealed class HelloHandler : IHandler
{
    public object Execute(JsonNode? @params) => new
    {
        hello = "world"
    };
}
