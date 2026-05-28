using System.Text.Json.Nodes;
using RhinoCli.Server;
using RhinoCli.Server.Reflection;

namespace RhinoCliPlugin;

[HandlerMetadataAttribute(
    "Inspect a .NET type by fully qualified name (FQN) and return its constructors, properties, methods (grouped by overload), events, and fields as structured JSON. The handler uses System.Reflection against the assemblies currently loaded in the Rhino process, so plugin-added types are covered automatically. Resolution is FQN-only (e.g. `Rhino.Geometry.Box`); use `rhino.search_types` first if you only know a short name. XML doc `<summary>` is attached to the type, each member, and each parameter when an `<AssemblyName>.xml` file (e.g. `RhinoCommon.xml`) is found beside the loaded assembly.",
    ParamsSchema = "{ name: string, binding?: \"public\" | \"public_instance\" | \"public_static\" | \"non_public\" | \"all\", include_inherited?: boolean }",
    ResultSchema = "{ full_name: string, assembly: string, kind: \"class\" | \"struct\" | \"interface\" | \"enum\", is_abstract: boolean, is_sealed: boolean, base_type: string?, interfaces: string[], constructors: object[], properties: object[], methods: object[], events: object[], fields: object[] }",
    Examples = new[]
    {
        "rhino-cli inspect-type System.String",
        "rhino-cli inspect-type Rhino.Geometry.Box",
        "rhino-cli call rhino.inspect_type '{\"name\":\"Rhino.Geometry.Box\",\"binding\":\"public\"}'"
    },
    DedicatedCommand = "rhino-cli inspect-type <FQN> [--binding <B>] [--include-inherited]",
    Category = "rhino")]
public sealed class InspectTypeHandler : IHandler
{
    public object Execute(JsonNode? @params)
    {
        var name = @params?["name"]?.GetValue<string>();
        if (string.IsNullOrWhiteSpace(name))
        {
            throw new RpcException(
                -32602,
                "Invalid params",
                new { field = "name", expected = "fully qualified type name" });
        }

        var binding = TypeInspector.ParseBinding(@params?["binding"]?.GetValue<string>());
        var includeInherited = @params?["include_inherited"]?.GetValue<bool>() ?? false;

        return TypeInspector.Inspect(name.Trim(), new InspectOptions(binding, includeInherited));
    }
}
