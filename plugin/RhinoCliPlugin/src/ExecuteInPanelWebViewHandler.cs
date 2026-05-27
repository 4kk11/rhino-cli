using System.Text.Json;
using System.Text.Json.Nodes;
using Eto.Forms;
using Rhino;
using Rhino.UI;
using RhinoCli.Server;

namespace RhinoCliPlugin;

[HandlerMetadataAttribute(
    "Execute JavaScript inside the first Eto.Forms.WebView found under a panel (resolved by GUID). The handler walks the panel's Eto control tree depth-first, wraps the user script in an IIFE so `return` is valid, and serializes the JS return value via JSON.stringify on the JS side. Use this instead of reflection-poking the plugin's private `_webView` field. The panel must have been displayed at least once for `Rhino.UI.Panels.GetPanel` to return its instance.",
    ParamsSchema = "{ panel: string (GUID), script: string }",
    ResultSchema = "{ status: \"ok\" | \"panel_not_found\" | \"webview_not_found\" | \"execution_error\", value?: any, error?: string, stack?: string, panel_type?: string }",
    Examples = new[]
    {
        "rhino-cli execute-panel-js F2A3B4C5-D6E7-8901-ABCD-EF0123456789 'return document.readyState'",
        "rhino-cli execute-panel-js <GUID> 'return JSON.stringify({title: document.title, url: location.href})'"
    },
    DedicatedCommand = "rhino-cli execute-panel-js <GUID> <SCRIPT>",
    SideEffects = "Runs arbitrary JavaScript inside the panel's WebView. May mutate DOM, fire navigation, or trigger any JS-exposed plugin API.",
    Category = "rhino")]
public sealed class ExecuteInPanelWebViewHandler : IHandler
{
    private const int ExecuteScriptTimeoutSeconds = 10;

    public object Execute(JsonNode? @params)
    {
        var panelGuidRaw = @params?["panel"]?.GetValue<string>();
        var script = @params?["script"]?.GetValue<string>();
        if (string.IsNullOrWhiteSpace(panelGuidRaw) || string.IsNullOrWhiteSpace(script))
        {
            throw new RpcException(
                -32602,
                "Invalid params",
                new { reason = "both `panel` (GUID) and `script` are required" });
        }

        if (!Guid.TryParse(panelGuidRaw, out var panelGuid))
        {
            throw new RpcException(
                -32602,
                "Invalid params",
                new { field = "panel", expected = "GUID string", actual = panelGuidRaw });
        }

        // The generic GetPanel<T> overload requires the panel's CLR type, which the caller does not know.
        // GUID-keyed lookup is the only entry point the AI agent can use.
#pragma warning disable CS0618
        var panel = Panels.GetPanel(panelGuid);
#pragma warning restore CS0618
        if (panel == null)
        {
            return new
            {
                status = "panel_not_found",
                error = $"Panels.GetPanel returned null for {panelGuid}. The panel must have been displayed at least once.",
            };
        }

        if (panel is not Control control)
        {
            return new
            {
                status = "panel_not_found",
                error = $"Panel instance is not an Eto.Forms.Control (got {panel.GetType().FullName}).",
                panel_type = panel.GetType().FullName,
            };
        }

        var webView = FindFirstWebView(control);
        if (webView == null)
        {
            return new
            {
                status = "webview_not_found",
                error = "No Eto.Forms.WebView was found in the panel's control tree.",
                panel_type = panel.GetType().FullName,
            };
        }

        var wrapped = WrapScript(script!);
        // WKWebView on macOS evaluates JS asynchronously, so the synchronous
        // ExecuteScript overload returns null/empty. Use the async overload
        // and pump the Eto run loop on the UI thread until the Task completes.
        var task = webView.ExecuteScriptAsync(wrapped);
        var deadline = DateTime.UtcNow.AddSeconds(ExecuteScriptTimeoutSeconds);
        while (!task.IsCompleted)
        {
            if (DateTime.UtcNow >= deadline)
            {
                return new
                {
                    status = "execution_error",
                    error = $"WebView.ExecuteScriptAsync did not complete within {ExecuteScriptTimeoutSeconds}s. The script may be blocking or the WebView is not responsive.",
                    panel_type = panel.GetType().FullName,
                };
            }
            Application.Instance.RunIteration();
        }

        if (task.IsFaulted)
        {
            return new
            {
                status = "execution_error",
                error = task.Exception?.GetBaseException().Message ?? "ExecuteScriptAsync faulted",
                panel_type = panel.GetType().FullName,
            };
        }

        var raw = task.Result;
        if (string.IsNullOrEmpty(raw))
        {
            return new
            {
                status = "execution_error",
                error = "ExecuteScriptAsync returned an empty string. The script may have thrown before the wrapper could capture it, or the WebView is not ready.",
                panel_type = panel.GetType().FullName,
            };
        }

        return ParseWrappedResult(raw, panel.GetType().FullName);
    }

    private static WebView? FindFirstWebView(Control control)
    {
        if (control is WebView wv)
        {
            return wv;
        }

        if (control is Container container)
        {
            foreach (var child in container.Children)
            {
                if (child == null)
                {
                    continue;
                }
                var found = FindFirstWebView(child);
                if (found != null)
                {
                    return found;
                }
            }
        }

        return null;
    }

    private static string WrapScript(string userScript)
    {
        // Eto's macOS WebView wraps the script in a function body and extracts
        // the value via `return`, so the outer expression must start with `return`.
        // The inner IIFE provides a function scope so the user's `return` is valid.
        // JSON.stringify captures the value; thrown errors land in the catch.
        return
            "return (function(){try{var __v=(function(){"
            + userScript
            + "\n})();return JSON.stringify({__ok:true,value:(typeof __v==='undefined')?null:__v});}"
            + "catch(__e){return JSON.stringify({__ok:false,error:String(__e),stack:(__e&&__e.stack)||null});}})()";
    }

    private static object ParseWrappedResult(string raw, string? panelType)
    {
        JsonNode? parsed;
        try
        {
            parsed = JsonNode.Parse(raw);
        }
        catch (Exception ex)
        {
            return new
            {
                status = "execution_error",
                error = $"Failed to parse WebView result as JSON: {ex.Message}",
                raw,
                panel_type = panelType,
            };
        }

        if (parsed is not JsonObject obj)
        {
            return new
            {
                status = "execution_error",
                error = "WebView wrapper returned a non-object JSON value.",
                raw,
                panel_type = panelType,
            };
        }

        var ok = obj["__ok"]?.GetValue<bool>() ?? false;
        if (!ok)
        {
            return new
            {
                status = "execution_error",
                error = obj["error"]?.GetValue<string>() ?? "Unknown JS error",
                stack = obj["stack"]?.GetValue<string?>(),
                panel_type = panelType,
            };
        }

        return new Dictionary<string, object?>
        {
            ["status"] = "ok",
            ["value"] = obj["value"],
            ["panel_type"] = panelType,
        };
    }
}
