using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.IO;
using System.Linq;
using System.Text.Json.Nodes;
using Rhino;
using Rhino.Display;
using Rhino.Geometry;
using RhinoCli.Server;

namespace RhinoCliPlugin;

[HandlerMetadataAttribute(
    "Capture a single Rhino viewport to a PNG. Optionally set display mode, projection, camera/target, or zoom-extents before capture. Runs on the UI thread. Display mode is applied via the CaptureToBitmap(Size, DisplayModeDescription) overload so viewport DisplayMode is not mutated; camera/projection/zoom_extents do mutate the target view (state is not restored — idempotent re-capture is preferred).",
    ParamsSchema = "{ width: int, height: int, viewport?: string, mode?: string, projection?: \"perspective\" | \"parallel\", camera?: [number, number, number], target?: [number, number, number], zoom_extents?: bool, transparent_background?: bool }",
    ResultSchema = "{ status: string, png_base64: string, format: \"png\", width: int, height: int, viewport: string, mode_applied: string }",
    Examples = new[]
    {
        "rhino-cli capture-viewport --width 1280 --height 720 --mode Shaded",
        "rhino-cli capture-viewport --viewport Perspective --width 800 --height 600 --camera 50,-80,40 --target 0,0,5 --mode Rendered",
        "rhino-cli call rhino.capture_viewport '{\"width\":800,\"height\":600,\"mode\":\"shaded\",\"zoom_extents\":true}'"
    },
    DedicatedCommand = "rhino-cli capture-viewport [--viewport NAME] --width N --height N [--mode M] [--projection perspective|parallel] [--camera X,Y,Z] [--target X,Y,Z] [--zoom-extents] [--transparent] [--out FILE]",
    SideEffects = "Mutates the target view's projection, camera, and (if zoom_extents) zoom. Does not change the view's DisplayMode (capture uses an overload that takes the mode as an argument). State is not restored.",
    Category = "rhino")]
public sealed class CaptureViewportHandler : IHandler
{
    public object Execute(JsonNode? @params)
    {
        // 1. width / height
        var widthNode = @params?["width"];
        var heightNode = @params?["height"];
        if (widthNode is null || heightNode is null)
            throw new RpcException(-32602, "Invalid params",
                new { field = "width|height", expected = "both integer > 0" });

        int width = widthNode.GetValue<int>();
        int height = heightNode.GetValue<int>();
        if (width <= 0 || height <= 0)
            throw new RpcException(-32602, "Invalid params",
                new { field = "width|height", expected = "integer > 0", got = new { width, height } });

        // 2. optional params
        string? viewportName = @params?["viewport"]?.GetValue<string>();
        string? modeName = @params?["mode"]?.GetValue<string>();
        string? projection = @params?["projection"]?.GetValue<string>();
        var camera = ReadXyz(@params, "camera");
        var target = ReadXyz(@params, "target");
        bool zoomExtents = @params?["zoom_extents"]?.GetValue<bool>() ?? false;
        bool transparentBg = @params?["transparent_background"]?.GetValue<bool>() ?? false;

        // 3. mutual exclusion
        if ((camera != null || target != null) && zoomExtents)
            throw new RpcException(-32602, "Invalid params",
                new
                {
                    field = "camera|target|zoom_extents",
                    expected = "either explicit camera/target OR zoom_extents=true, not both"
                });

        // 4. resolve doc & view
        var doc = RhinoDoc.ActiveDoc
            ?? throw new RpcException(-32000, "No active Rhino document", new { });

        RhinoView view;
        if (string.IsNullOrWhiteSpace(viewportName))
        {
            view = doc.Views.ActiveView
                ?? throw new RpcException(-32000, "No active view", new { });
        }
        else
        {
            view = doc.Views.Find(viewportName!, false)
                ?? throw new RpcException(-32602, "Invalid params",
                    new
                    {
                        field = "viewport",
                        expected = "existing viewport name",
                        got = viewportName,
                        available = doc.Views.Select(v => v.ActiveViewport.Name).ToArray()
                    });
        }

        // 5. resolve display mode
        DisplayModeDescription? modeDesc = null;
        if (!string.IsNullOrWhiteSpace(modeName))
        {
            modeDesc = DisplayModeDescription.FindByName(modeName!);
            if (modeDesc == null)
            {
                // LocalName fallback for non-English locales
                var all = DisplayModeDescription.GetDisplayModes();
                modeDesc = all.FirstOrDefault(m =>
                    string.Equals(m.LocalName, modeName, StringComparison.OrdinalIgnoreCase));
                if (modeDesc == null)
                    throw new RpcException(-32602, "Invalid params",
                        new
                        {
                            field = "mode",
                            expected = "display mode name (English or local)",
                            got = modeName,
                            available = all.Select(m => m.EnglishName).ToArray()
                        });
            }
        }

        // 6. apply mutations: projection → camera → target → zoom_extents
        var vp = view.ActiveViewport;
        if (!string.IsNullOrWhiteSpace(projection))
        {
            bool isParallel = projection!.Equals("parallel", StringComparison.OrdinalIgnoreCase);
            bool isPerspective = projection!.Equals("perspective", StringComparison.OrdinalIgnoreCase);
            if (!isParallel && !isPerspective)
                throw new RpcException(-32602, "Invalid params",
                    new { field = "projection", expected = "\"perspective\" | \"parallel\"", got = projection });
            if (isParallel)
                vp.ChangeToParallelProjection(true);
            else
                vp.ChangeToPerspectiveProjection(true, 50.0);
        }
        if (camera != null)
            vp.SetCameraLocation(new Point3d(camera[0], camera[1], camera[2]), false);
        if (target != null)
            vp.SetCameraTarget(new Point3d(target[0], target[1], target[2]), false);
        if (zoomExtents)
            vp.ZoomExtents();

        view.Redraw();

        // 7. capture
        var size = new Size(width, height);
        Bitmap bitmap;
        string modeAppliedName;

        if (transparentBg)
        {
            // DisplayPipelineAttributes constructors are internal; obtain via DisplayModeDescription.DisplayAttributes.
            // Temporarily set FillMode = Transparent, capture, then restore (UI thread, safe).
            var baseMode = modeDesc ?? vp.DisplayMode;
            var attrs = baseMode?.DisplayAttributes;
            if (attrs != null)
            {
                var savedFillMode = attrs.FillMode;
                attrs.FillMode = DisplayPipelineAttributes.FrameBufferFillMode.Transparent;
                try
                {
                    bitmap = view.CaptureToBitmap(size, attrs)
                        ?? throw new RpcException(-32000, "CaptureToBitmap returned null",
                            new { width, height, viewport = vp.Name });
                }
                finally
                {
                    attrs.FillMode = savedFillMode;
                }
                modeAppliedName = baseMode?.EnglishName ?? "";
            }
            else
            {
                bitmap = view.CaptureToBitmap(size)
                    ?? throw new RpcException(-32000, "CaptureToBitmap returned null",
                        new { width, height, viewport = vp.Name });
                modeAppliedName = vp.DisplayMode?.EnglishName ?? "";
            }
        }
        else if (modeDesc != null)
        {
            bitmap = view.CaptureToBitmap(size, modeDesc)
                ?? throw new RpcException(-32000, "CaptureToBitmap returned null",
                    new { width, height, viewport = vp.Name });
            modeAppliedName = modeDesc.EnglishName ?? "";
        }
        else
        {
            bitmap = view.CaptureToBitmap(size)
                ?? throw new RpcException(-32000, "CaptureToBitmap returned null",
                    new { width, height, viewport = vp.Name });
            modeAppliedName = vp.DisplayMode?.EnglishName ?? "";
        }

        // 8. PNG encode + base64
        string base64;
        using (bitmap)
        using (var stream = new MemoryStream())
        {
            bitmap.Save(stream, ImageFormat.Png);
            base64 = Convert.ToBase64String(stream.ToArray());
        }

        return new
        {
            status = "ok",
            png_base64 = base64,
            format = "png",
            width,
            height,
            viewport = vp.Name ?? "",
            mode_applied = modeAppliedName
        };
    }

    private static double[]? ReadXyz(JsonNode? @params, string field)
    {
        var node = @params?[field];
        if (node is null) return null;
        if (node is not JsonArray arr || arr.Count != 3)
            throw new RpcException(-32602, "Invalid params",
                new { field, expected = "array of three numbers [x, y, z]" });
        try
        {
            return new[]
            {
                arr[0]!.GetValue<double>(),
                arr[1]!.GetValue<double>(),
                arr[2]!.GetValue<double>()
            };
        }
        catch (Exception ex)
        {
            throw new RpcException(-32602, "Invalid params",
                new { field, expected = "array of three numbers", reason = ex.Message });
        }
    }
}
