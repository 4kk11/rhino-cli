using System.Text.Json.Nodes;
using RhinoCli.Server;
using RhinoCli.Server.Reflection;

namespace RhinoCliPlugin;

[HandlerMetadataAttribute(
    "Search loaded .NET assemblies for types and members whose name contains the given substring (case-insensitive). Use this to discover fully qualified type names before calling `rhino.inspect_type`, or to find which class hosts a method like `AddBox`. By default the search is restricted to assemblies whose name starts with `Rhino`, `RhinoCommon`, or `RhinoCli`; pass `assembly` to widen or change the filter. Results are capped by `limit` (default 50) and `truncated` indicates more matches were skipped.",
    ParamsSchema = "{ pattern: string, scope?: \"all\" | \"types\" | \"members\", assembly?: string, limit?: int }",
    ResultSchema = "{ matches: { kind: \"type\" | \"method\" | \"property\" | \"field\" | \"event\" | \"constructor\", full_name: string, member?: string, assembly: string }[], truncated: boolean }",
    Examples = new[]
    {
        "rhino-cli search-types AddBox",
        "rhino-cli search-types Box --scope types",
        "rhino-cli call rhino.search_types '{\"pattern\":\"AddBox\",\"limit\":20}'"
    },
    DedicatedCommand = "rhino-cli search-types <PATTERN> [--scope <S>] [--assembly <NAME>] [--limit <N>]",
    Category = "rhino")]
public sealed class SearchTypesHandler : IHandler
{
    public object Execute(JsonNode? @params)
    {
        var pattern = @params?["pattern"]?.GetValue<string>();
        if (string.IsNullOrWhiteSpace(pattern))
        {
            throw new RpcException(
                -32602,
                "Invalid params",
                new { field = "pattern", expected = "non-empty substring" });
        }

        var scope = TypeSearcher.ParseScope(@params?["scope"]?.GetValue<string>());
        var assemblyFilter = @params?["assembly"]?.GetValue<string>();
        var limit = @params?["limit"]?.GetValue<int>() ?? 50;

        var result = TypeSearcher.Search(new SearchOptions(pattern.Trim(), scope, assemblyFilter, limit));

        return new
        {
            matches = result.Matches.Select(m => new
            {
                kind = m.Kind,
                full_name = m.FullName,
                member = m.Member,
                assembly = m.Assembly,
            }).ToArray(),
            truncated = result.Truncated,
        };
    }
}
