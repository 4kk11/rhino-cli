using System.Text.Json.Nodes;
using Rhino;
using RhinoCli.Server;

namespace RhinoCliPlugin;

[HandlerMetadataAttribute(
    "Run a Rhino command script on the Rhino UI thread.",
    ParamsSchema = "{ script: string, echo?: boolean, mru_display_string?: string }",
    ResultSchema = "{ status: string, success: boolean, script: string, command_prompt: string }",
    Examples = new[]
    {
        "rhino-cli run-script \"_Zoom _Extents\"",
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
