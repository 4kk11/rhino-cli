using System.Text.Json.Nodes;
using Rhino;
using Rhino.Commands;
using RhinoCli.Server;

namespace RhinoCliPlugin;

[HandlerMetadataAttribute(
    "Probe a Rhino command by starting it and immediately sending Cancel several times, then return the first Get prompt (with option labels) as it appeared on the command line. Useful for discovering a command's argument syntax dynamically when the spec cannot be inferred from name alone. The captured prompt is in Rhino's current locale; option short codes in parens (e.g. `(D)`, `(P)`) are ASCII-stable. `prompt_changed=false` means the probe could not update CommandPrompt — the command may have hung or failed to start.",
    ParamsSchema = "{ name: string }",
    ResultSchema = "{ exists: boolean, input_name: string, english_name?: string, script: string, run_script_success: boolean, prompt_changed: boolean, first_prompt: string, prompt_before: string }",
    Examples = new[]
    {
        "rhino-cli probe-command Box",
        "rhino-cli probe-command Sphere",
        "rhino-cli call rhino.probe_command '{\"name\":\"Line\"}'"
    },
    DedicatedCommand = "rhino-cli probe-command <NAME>",
    SideEffects = "Briefly starts the target command on the Rhino UI thread and sends Cancel multiple times. Commands with immediate side effects (e.g., layer/view changes triggered before the first Get prompt) may still occur. Commands whose Get loop interprets `_Cancel` as a string (e.g. some curve-selection prompts) can hang the UI thread until the script eventually unwinds.",
    Category = "rhino")]
public sealed class ProbeCommandHandler : IHandler
{
    private const int CancelRepeats = 5;

    public object Execute(JsonNode? @params)
    {
        var name = @params?["name"]?.GetValue<string>();
        if (string.IsNullOrWhiteSpace(name))
        {
            throw new RpcException(
                -32602,
                "Invalid params",
                new { field = "name", expected = "non-empty string" });
        }

        var trimmed = name.Trim();
        var commandId = Command.LookupCommandId(trimmed, searchForEnglishName: true);
        if (commandId == Guid.Empty)
        {
            commandId = Command.LookupCommandId(trimmed, searchForEnglishName: false);
        }

        if (commandId == Guid.Empty)
        {
            return new
            {
                exists = false,
                input_name = trimmed,
                script = "",
                run_script_success = false,
                prompt_changed = false,
                first_prompt = "",
                prompt_before = ""
            };
        }

        var englishName = Command.LookupCommandName(commandId, englishName: true) ?? trimmed;
        var cancelTail = string.Join(" ", Enumerable.Repeat("_Cancel", CancelRepeats));
        var script = $"! _-{englishName} {cancelTail}";

        var promptBefore = RhinoApp.CommandPrompt ?? "";
        var success = RhinoApp.RunScript(script, echo: false);
        var promptAfter = RhinoApp.CommandPrompt ?? "";
        var promptChanged = !string.Equals(promptBefore, promptAfter, StringComparison.Ordinal);

        RhinoCliHistoryBuffer.Append($"rhino-cli probe-command: {englishName}");

        return new
        {
            exists = true,
            input_name = trimmed,
            english_name = englishName,
            script,
            run_script_success = success,
            prompt_changed = promptChanged,
            first_prompt = promptChanged ? promptAfter : "",
            prompt_before = promptBefore
        };
    }

}
