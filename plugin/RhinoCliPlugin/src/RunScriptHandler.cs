using System.Reflection;
using System.Text.Json.Nodes;
using Rhino;
using RhinoCli.Server;

namespace RhinoCliPlugin;

[HandlerMetadataAttribute(
    "Run a Rhino command script on the Rhino UI thread. Syntax: `_-Command arg1 arg2 ...`. `_` prefix = English-locale name (recommended), `-` prefix = suppress dialog, `!` prefix = cancel any active command first. Coordinates use commas (e.g. `0,0,0`); spaces between args act as Enter. The result includes `objects_added`/`objects_removed` from the active document and `history_delta` (new lines emitted during execution) so callers can verify outcomes without a separate history call. `success` reflects script parsing only; trust the deltas to determine whether the underlying command actually did what you wanted.",
    ParamsSchema = "{ script: string, echo?: boolean, mru_display_string?: string }",
    ResultSchema = "{ status: string, success: boolean, script: string, command_prompt: string, command_prompt_changed: boolean, objects_added: int, objects_removed: int, object_count_before: int, object_count_after: int, history_delta: string[] }",
    Examples = new[]
    {
        "rhino-cli run-script \"_Zoom _Extents\"",
        "rhino-cli run-script \"! _-Box 0,0,0 10,10,10\"",
        "rhino-cli call rhino.run_script '{\"script\":\"_Zoom _Extents\",\"echo\":false}'"
    },
    DedicatedCommand = "rhino-cli run-script <SCRIPT>",
    SideEffects = "Executes Rhino commands and may modify the active document.",
    Category = "rhino")]
public sealed class RunScriptHandler : IHandler
{
    public object Execute(JsonNode? @params)
    {
        var script = @params?["script"]?.GetValue<string>();
        if (string.IsNullOrWhiteSpace(script))
        {
            throw new RpcException(
                -32602,
                "Invalid params",
                new { field = "script", expected = "non-empty string" });
        }

        var echo = @params?["echo"]?.GetValue<bool>() ?? false;
        var mruDisplayString = @params?["mru_display_string"]?.GetValue<string>();

        var doc = RhinoDoc.ActiveDoc;
        var objectCountBefore = doc?.Objects.Count ?? 0;
        var historyBefore = SnapshotHistoryLines();
        var promptBefore = RhinoApp.CommandPrompt ?? "";

        var success = string.IsNullOrWhiteSpace(mruDisplayString)
            ? RhinoApp.RunScript(script, echo)
            : RhinoApp.RunScript(script, mruDisplayString, echo);

        var historyAfter = SnapshotHistoryLines();
        var objectCountAfter = (RhinoDoc.ActiveDoc ?? doc)?.Objects.Count ?? 0;
        var promptAfter = RhinoApp.CommandPrompt ?? "";

        var historyDelta = ComputeHistoryDelta(historyBefore, historyAfter, script);

        RhinoCliHistoryBuffer.Append($"rhino-cli run-script: {script}");
        RhinoCliHistoryBuffer.Append($"rhino-cli run-script result: {success}");
        RhinoApp.WriteLine($"rhino-cli run-script: {script}");
        RhinoApp.WriteLine($"rhino-cli run-script result: {success}");

        var added = Math.Max(0, objectCountAfter - objectCountBefore);
        var removed = Math.Max(0, objectCountBefore - objectCountAfter);

        return new
        {
            status = success ? "ok" : "failed",
            success,
            script,
            command_prompt = promptAfter,
            command_prompt_changed = !string.Equals(promptBefore, promptAfter, StringComparison.Ordinal),
            objects_added = added,
            objects_removed = removed,
            object_count_before = objectCountBefore,
            object_count_after = objectCountAfter,
            history_delta = historyDelta,
        };
    }

    private static string[] SnapshotHistoryLines()
    {
        var etoText = TryGetEtoCommandHistoryText();
        var fallback = RhinoApp.CommandHistoryWindowText ?? "";
        var text = string.IsNullOrEmpty(etoText) ? fallback : etoText;
        if (string.IsNullOrEmpty(text))
        {
            text = RhinoCliHistoryBuffer.Text();
        }
        text = text.Replace("\r\n", "\n").Replace('\r', '\n');
        if (string.IsNullOrEmpty(text))
        {
            return Array.Empty<string>();
        }
        var lines = text.Split('\n');
        if (lines.Length > 0 && lines[^1].Length == 0)
        {
            return lines.Take(lines.Length - 1).ToArray();
        }
        return lines;
    }

    private static string[] ComputeHistoryDelta(string[] before, string[] after, string script)
    {
        var commonLength = Math.Min(before.Length, after.Length);
        int divergeIndex = 0;
        while (divergeIndex < commonLength && before[divergeIndex] == after[divergeIndex])
        {
            divergeIndex++;
        }

        // If the histories diverge mid-stream (e.g. ring-buffer rotation), fall back to
        // tail-only comparison: drop everything in `after` whose index has matching content in `before`.
        IEnumerable<string> deltaSource;
        if (divergeIndex == before.Length)
        {
            deltaSource = after.Skip(before.Length);
        }
        else
        {
            deltaSource = after.Skip(divergeIndex);
        }

        // Filter out the bookkeeping lines we just appended ourselves so the delta represents
        // what the user-issued script actually emitted.
        var ownPrefixes = new[]
        {
            $"rhino-cli run-script: {script}",
            "rhino-cli run-script result:",
        };
        return deltaSource
            .Where(line => !ownPrefixes.Any(prefix => line.StartsWith(prefix, StringComparison.Ordinal)))
            .ToArray();
    }

    private static string? TryGetEtoCommandHistoryText()
    {
        var viewModel = CommandHistoryHandler.GetEtoCommandHistoryViewModel();
        return viewModel?.GetType()
            .GetProperty(
                "CommandText",
                BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic)
            ?.GetValue(viewModel) as string;
    }
}
