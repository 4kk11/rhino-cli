using System.Text.Json.Nodes;
using RhinoCli.Server;
using RhinoCli.Server.Reflection;

namespace RhinoCliPlugin;

[HandlerMetadataAttribute(
    "Decompile a single .NET method's IL into C# source via ICSharpCode.Decompiler. Useful when the signature returned by `rhino.inspect_type` is not enough and you want to understand what the method actually does (control flow, helper calls, edge cases). Resolution is by type FQN + method name; if the name is overloaded, pass `signature` as a comma-separated list of parameter type names (FullName or short Name accepted) to disambiguate. Use `.ctor` for constructors.",
    ParamsSchema = "{ type: string, method: string, signature?: string }",
    ResultSchema = "{ type: string, method: string, signature: string, csharp: string, summary: string }",
    Examples = new[]
    {
        "rhino-cli decompile-method Rhino.Geometry.Box ClosestPoint",
        "rhino-cli decompile-method Rhino.Geometry.Box ClosestPoint --signature Point3d",
        "rhino-cli call rhino.decompile_method '{\"type\":\"Rhino.Geometry.Box\",\"method\":\"ClosestPoint\"}'"
    },
    DedicatedCommand = "rhino-cli decompile-method <TYPE> <METHOD> [--signature <SIG>]",
    Category = "rhino")]
public sealed class DecompileMethodHandler : IHandler
{
    public object Execute(JsonNode? @params)
    {
        var typeName = @params?["type"]?.GetValue<string>();
        var methodName = @params?["method"]?.GetValue<string>();
        var signature = @params?["signature"]?.GetValue<string>();

        var result = MethodDecompiler.Decompile(typeName ?? "", methodName ?? "", signature);

        return new
        {
            type = result.Type,
            method = result.Method,
            signature = result.Signature,
            csharp = result.CSharp,
            summary = result.Summary,
        };
    }
}
