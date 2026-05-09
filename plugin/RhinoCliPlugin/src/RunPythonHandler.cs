using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Rhino;
using Rhino.Runtime;
using RhinoCli.Server;

namespace RhinoCliPlugin;

[HandlerMetadataAttribute(
    "Execute an inline Python script string against the active Rhino document. The plugin treats this as the official escape hatch for arbitrary geometry / attribute / RhinoCommon work — when something cannot be expressed via `run_script`, `list_objects`, `delete_objects`, etc., write Python here instead of asking for a new bespoke handler. `scriptcontext.doc` is set to the active document. `print()` output is captured and returned as `stdout`. Provide `result_expression` to evaluate a final expression after the source statements; the value is JSON-serialized into `result` (primitives wrapped directly, other values via `System.Text.Json` then a `repr()` fallback). Idiomatic structured return: assemble a dict in source, then set `result_expression` to a `json.dumps(...)` call so the caller receives a parseable string.",
    ParamsSchema = "{ source: string, result_expression?: string }",
    ResultSchema = "{ status: string, success: bool, stdout: string, result?: any, result_repr?: string, error?: string }",
    Examples = new[]
    {
        "rhino-cli call rhino.run_python '{\"source\":\"import scriptcontext as sc\\nprint(sc.doc.Objects.Count)\"}'",
        "rhino-cli call rhino.run_python '{\"source\":\"\",\"result_expression\":\"2 + 2\"}'",
        "rhino-cli call rhino.run_python '{\"source\":\"import scriptcontext as sc, json\\nids = [str(o.Id) for o in sc.doc.Objects]\",\"result_expression\":\"json.dumps(ids)\"}'",
    },
    DedicatedCommand = "rhino-cli run-python <SOURCE> [--result-expression <EXPR>]",
    SideEffects = "Executes arbitrary Python and may modify the active Rhino document.",
    Category = "rhino")]
public sealed class RunPythonHandler : IHandler
{
    public object Execute(JsonNode? @params)
    {
        var source = @params?["source"]?.GetValue<string>() ?? "";
        var resultExpression = @params?["result_expression"]?.GetValue<string>();
        if (string.IsNullOrEmpty(source) && string.IsNullOrEmpty(resultExpression))
        {
            throw new RpcException(
                -32602,
                "Invalid params",
                new { reason = "either `source` or `result_expression` must be non-empty" });
        }

        var python = PythonScript.Create()
                     ?? throw new RpcException(
                         -32000,
                         "Python plug-in is not available in this Rhino instance",
                         new { });

        var stdout = new StringBuilder();
        python.Output = text => stdout.Append(text);
        python.ScriptContextDoc = RhinoDoc.ActiveDoc;
        python.SetupScriptContext(RhinoDoc.ActiveDoc);

        bool success;
        string? error = null;
        object? rawResult = null;
        bool evaluatedExpression = false;

        try
        {
            if (!string.IsNullOrEmpty(resultExpression))
            {
                rawResult = python.EvaluateExpression(source, resultExpression);
                evaluatedExpression = true;
                success = true;
            }
            else
            {
                success = python.ExecuteScript(source);
            }
        }
        catch (Exception ex)
        {
            success = false;
            error = python.GetStackTraceFromException(ex) ?? ex.Message;
        }

        var output = stdout.ToString();
        var historyMessage = success
            ? "rhino-cli run-python: ok"
            : $"rhino-cli run-python: error {(error ?? "execution returned false")}";
        RhinoCliHistoryBuffer.Append(historyMessage);
        RhinoApp.WriteLine(historyMessage);

        var response = new Dictionary<string, object?>
        {
            ["status"] = success ? "ok" : "failed",
            ["success"] = success,
            ["stdout"] = output,
            ["error"] = error,
        };

        if (evaluatedExpression)
        {
            var (node, repr) = ConvertResult(rawResult);
            response["result"] = node;
            response["result_repr"] = repr;
        }

        return response;
    }

    private static (JsonNode? node, string? repr) ConvertResult(object? raw)
    {
        if (raw is null)
        {
            return (null, "None");
        }

        var repr = raw.ToString() ?? "";

        switch (raw)
        {
            case bool b:
                return (JsonValue.Create(b), repr);
            case sbyte or byte or short or ushort or int or uint:
                return (JsonValue.Create(Convert.ToInt64(raw)), repr);
            case long l:
                return (JsonValue.Create(l), repr);
            case ulong ul:
                return (JsonValue.Create((double)ul), repr);
            case float f:
                return (JsonValue.Create(f), repr);
            case double d:
                return (JsonValue.Create(d), repr);
            case decimal dec:
                return (JsonValue.Create((double)dec), repr);
            case string s:
                return (JsonValue.Create(s), repr);
        }

        try
        {
            var json = JsonSerializer.Serialize(raw);
            return (JsonNode.Parse(json), repr);
        }
        catch
        {
            return (JsonValue.Create(repr), repr);
        }
    }
}
