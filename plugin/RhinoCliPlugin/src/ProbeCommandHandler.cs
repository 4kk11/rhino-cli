using System.Text.Json.Nodes;
using Rhino;
using Rhino.Commands;
using RhinoCli.Server;

namespace RhinoCliPlugin;

[HandlerMetadataAttribute(
    "Probe a Rhino command by starting it and immediately canceling, then return the first Get prompt and any Write/WriteLine output captured from the command window. Useful for discovering a command's argument syntax dynamically when the spec cannot be inferred from name alone. Output is in Rhino's current locale; option short codes in parens (e.g. `(D)`, `(P)`) are ASCII-stable. `prompt_changed=false` means the probe could not update CommandPrompt — the command may have failed to start, did not enter a Get loop, or hung in a curve/object-selection Get that does not honor `_Cancel`.",
    ParamsSchema = "{ name: string }",
    ResultSchema = "{ exists: boolean, input_name: string, english_name?: string, script: string, run_script_success: boolean, prompt_changed: boolean, first_prompt: string, prompt_before: string, captured: string[], escape_sent: boolean }",
    Examples = new[]
    {
        "rhino-cli probe-command Box",
        "rhino-cli probe-command Chamfer",
        "rhino-cli call rhino.probe_command '{\"name\":\"Line\"}'"
    },
    DedicatedCommand = "rhino-cli probe-command <NAME>",
    SideEffects = "Briefly starts the target command on the Rhino UI thread and cancels via `_Cancel` tokens plus a fallback Escape keystroke (`RhinoApp.SendKeystrokes(\"\\u001b\")`) sent from a background thread when the command does not return within ~300ms. Calls `RhinoApp.CapturedCommandWindowStrings(true)` before and after RunScript to isolate this probe's Write/WriteLine output. Commands with immediate side effects (e.g., layer/view changes triggered before the first Get prompt) may still occur.",
    Category = "rhino")]
public sealed class ProbeCommandHandler : IHandler
{
    private const int CancelRepeats = 5;
    private const int EscapeFallbackDelayMs = 300;
    private const int EscapeJoinTimeoutMs = 500;
    private const string EscapeCharacter = "";

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
                prompt_before = "",
                captured = Array.Empty<string>(),
                escape_sent = false
            };
        }

        var englishName = Command.LookupCommandName(commandId, englishName: true) ?? trimmed;
        var cancelTail = string.Join(" ", Enumerable.Repeat("_Cancel", CancelRepeats));
        var script = $"! _-{englishName} {cancelTail}";

        var promptBefore = RhinoApp.CommandPrompt ?? "";

        // Drain any pending captured strings so we can isolate this probe's output.
        var captureWasEnabled = RhinoApp.CommandWindowCaptureEnabled;
        RhinoApp.CommandWindowCaptureEnabled = true;
        RhinoApp.CapturedCommandWindowStrings(clearBuffer: true);

        using var cts = new CancellationTokenSource();
        var escapeSent = false;
        var escapeTask = Task.Run(async () =>
        {
            try
            {
                await Task.Delay(EscapeFallbackDelayMs, cts.Token).ConfigureAwait(false);
                RhinoApp.SendKeystrokes(EscapeCharacter, appendReturn: false);
                Volatile.Write(ref escapeSent, true);
            }
            catch (OperationCanceledException)
            {
            }
        });

        bool success;
        try
        {
            success = RhinoApp.RunScript(script, echo: false);
        }
        finally
        {
            cts.Cancel();
            try
            {
                escapeTask.Wait(TimeSpan.FromMilliseconds(EscapeJoinTimeoutMs));
            }
            catch
            {
                // Background task exceptions are non-fatal for the probe result.
            }
        }

        var captured = RhinoApp.CapturedCommandWindowStrings(clearBuffer: true) ?? Array.Empty<string>();
        if (!captureWasEnabled)
        {
            RhinoApp.CommandWindowCaptureEnabled = false;
        }

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
            prompt_before = promptBefore,
            captured,
            escape_sent = Volatile.Read(ref escapeSent)
        };
    }
}
