using System.Text.Json.Nodes;
using Rhino;
using RhinoCli.Server;

namespace MinimalPlugin;

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
        var success = string.IsNullOrWhiteSpace(mruDisplayString)
            ? RhinoApp.RunScript(script, echo)
            : RhinoApp.RunScript(script, mruDisplayString, echo);
        RhinoCliHistoryBuffer.Append($"rhino-cli run-script: {script}");
        RhinoCliHistoryBuffer.Append($"rhino-cli run-script result: {success}");
        RhinoApp.WriteLine($"rhino-cli run-script: {script}");
        RhinoApp.WriteLine($"rhino-cli run-script result: {success}");

        return new
        {
            status = success ? "ok" : "failed",
            success,
            script,
            command_prompt = RhinoApp.CommandPrompt ?? ""
        };
    }
}
