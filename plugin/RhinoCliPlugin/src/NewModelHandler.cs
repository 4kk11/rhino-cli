using System.Text.Json.Nodes;
using Rhino;
using Rhino.ApplicationSettings;
using RhinoCli.Server;

namespace RhinoCliPlugin;

public sealed class NewModelHandler : IHandler
{
    public object Execute(JsonNode? @params)
    {
        var requestedTemplate = @params?["template"]?.GetValue<string>();
        var template = ResolveTemplate(requestedTemplate);
        if (!string.IsNullOrWhiteSpace(template) && !File.Exists(template))
        {
            throw new RpcException(
                -32602,
                "Invalid params",
                new { field = "template", expected = "existing .3dm file path" });
        }

        var doc = RhinoDoc.Create(template);
        if (doc == null)
        {
            throw new RpcException(
                -32000,
                "Failed to create new Rhino model",
                new { template = template ?? "" });
        }

        RhinoDoc.ActiveDoc = doc;
        doc.Views.Redraw();

        var templateUsed = doc.TemplateFileUsed ?? "";
        var message = string.IsNullOrWhiteSpace(requestedTemplate)
            ? $"rhino-cli new-model: {doc.RuntimeSerialNumber}"
            : $"rhino-cli new-model: {doc.RuntimeSerialNumber} from {requestedTemplate}";
        RhinoCliHistoryBuffer.Append(message);
        RhinoApp.WriteLine(message);

        return new
        {
            status = "ok",
            document = new
            {
                runtime_serial_number = doc.RuntimeSerialNumber,
                name = doc.Name ?? "",
                path = doc.Path ?? "",
                template = template ?? "",
                template_file_used = templateUsed,
                object_count = doc.Objects.Count
            }
        };
    }

    private static string? ResolveTemplate(string? requestedTemplate)
    {
        if (!string.IsNullOrWhiteSpace(requestedTemplate))
        {
            return requestedTemplate;
        }

        var defaultTemplate = FileSettings.TemplateFile;
        return string.IsNullOrWhiteSpace(defaultTemplate) ? null : defaultTemplate;
    }
}
