using System.Reflection;
using System.Text;
using System.Text.Json.Nodes;
using Rhino;
using RhinoCli.Server;

namespace MinimalPlugin;

public sealed class CommandHistoryHandler : IHandler
{
    public object Execute(JsonNode? @params)
    {
        var tail = @params?["tail"]?.GetValue<int?>();
        if (tail < 0)
        {
            throw new RpcException(
                -32602,
                "Invalid params",
                new { field = "tail", expected = "integer >= 0" });
        }

        var etoText = TryGetEtoCommandHistoryText();
        var fallbackText = RhinoApp.CommandHistoryWindowText ?? "";
        var text = NormalizeNewlines(string.IsNullOrEmpty(etoText) ? fallbackText : etoText);
        if (string.IsNullOrEmpty(text))
        {
            text = RhinoCliHistoryBuffer.Text();
        }
        var lines = SplitLines(text);
        var totalLineCount = lines.Length;
        var returnedLines = lines;

        if (tail.HasValue)
        {
            var skip = Math.Max(0, totalLineCount - tail.Value);
            returnedLines = lines.Skip(skip).ToArray();
        }

        return new
        {
            status = "ok",
            text = string.Join("\n", returnedLines),
            line_count = returnedLines.Length,
            total_line_count = totalLineCount,
            truncated = returnedLines.Length != totalLineCount,
            command_prompt = RhinoApp.CommandPrompt ?? ""
        };
    }

    private static string NormalizeNewlines(string text)
    {
        return text.Replace("\r\n", "\n").Replace('\r', '\n');
    }

    private static string[] SplitLines(string text)
    {
        if (string.IsNullOrEmpty(text))
        {
            return Array.Empty<string>();
        }

        var lines = text.Split('\n');
        return lines.Length > 0 && lines[^1].Length == 0
            ? lines.Take(lines.Length - 1).ToArray()
            : lines;
    }

    private static string? TryGetEtoCommandHistoryText()
    {
        var viewModel = GetEtoCommandHistoryViewModel();
        return viewModel?.GetType()
            .GetProperty(
                "CommandText",
                BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic)
            ?.GetValue(viewModel) as string;
    }

    internal static object? GetEtoCommandHistoryViewModel()
    {
        var doc = RhinoDoc.ActiveDoc;
        if (doc == null)
        {
            return null;
        }

        var type = AppDomain.CurrentDomain
            .GetAssemblies()
            .Select(assembly => assembly.GetType(
                "Rhino.UI.DialogPanels.CommandHistoryViewModel",
                throwOnError: false))
            .FirstOrDefault(t => t != null);
        if (type == null)
        {
            return null;
        }

        var get = type.GetMethods(BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic)
            .FirstOrDefault(m =>
            {
                if (m.Name != "Get")
                {
                    return false;
                }

                var parameters = m.GetParameters();
                return parameters.Length == 1 && parameters[0].ParameterType == typeof(RhinoDoc);
            });

        return get?.Invoke(null, new object[] { doc });
    }
}

public sealed class ClearCommandHistoryHandler : IHandler
{
    public object Execute(JsonNode? @params)
    {
        ClearEtoCommandHistory();
        RhinoApp.ClearCommandHistoryWindow();
        RhinoCliHistoryBuffer.Clear();
        return new { status = "ok" };
    }

    private static void ClearEtoCommandHistory()
    {
        var viewModel = CommandHistoryHandler.GetEtoCommandHistoryViewModel();
        if (viewModel == null)
        {
            return;
        }

        lock (viewModel)
        {
            if (viewModel.GetType()
                    .GetField("_commandText", BindingFlags.Instance | BindingFlags.NonPublic)
                    ?.GetValue(viewModel) is StringBuilder builder)
            {
                builder.Clear();
            }

            viewModel.GetType()
                .GetField("_needsNewLine", BindingFlags.Instance | BindingFlags.NonPublic)
                ?.SetValue(viewModel, false);
        }

        viewModel.GetType()
            .GetMethod("NotifyTextChanged", BindingFlags.Instance | BindingFlags.NonPublic)
            ?.Invoke(viewModel, null);
    }
}
