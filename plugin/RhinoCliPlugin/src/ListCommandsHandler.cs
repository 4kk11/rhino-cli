using System.Text.Json.Nodes;
using Rhino.Commands;
using RhinoCli.Server;

namespace RhinoCliPlugin;

[HandlerMetadataAttribute(
    "List Rhino command names known to the running Rhino instance. Pair with `rhino.probe_command` to dynamically discover each command's first prompt and option labels before invoking `rhino.run_script`.",
    ParamsSchema = "null | { pattern?: string, include_unloaded?: boolean }",
    ResultSchema = "{ commands: string[], total_count: number, filtered_count: number }",
    Examples = new[]
    {
        "rhino-cli list-commands",
        "rhino-cli list-commands --pattern Box",
        "rhino-cli list-commands --include-unloaded"
    },
    DedicatedCommand = "rhino-cli list-commands [--pattern <P>] [--include-unloaded]",
    Category = "rhino")]
public sealed class ListCommandsHandler : IHandler
{
    public object Execute(JsonNode? @params)
    {
        var pattern = @params?["pattern"]?.GetValue<string>();
        var includeUnloaded = @params?["include_unloaded"]?.GetValue<bool>() ?? false;

        var names = Command.GetCommandNames(english: true, loaded: !includeUnloaded) ?? Array.Empty<string>();
        var total = names.Length;

        IEnumerable<string> filtered = names;
        if (!string.IsNullOrEmpty(pattern))
        {
            filtered = names.Where(n => n.IndexOf(pattern, StringComparison.OrdinalIgnoreCase) >= 0);
        }

        var commands = filtered.OrderBy(n => n, StringComparer.OrdinalIgnoreCase).ToArray();

        return new
        {
            commands,
            total_count = total,
            filtered_count = commands.Length
        };
    }
}
