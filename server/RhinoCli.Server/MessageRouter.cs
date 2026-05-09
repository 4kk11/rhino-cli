using System.Text.Json;
using System.Text.Json.Nodes;

namespace RhinoCli.Server;

public sealed class MessageRouter
{
    private const string JsonRpcVersion = "2.0";

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase
    };

    private readonly HandlerRegistry _registry;
    private readonly Func<IHandler, JsonNode?, object?> _invokeHandler;

    public MessageRouter(HandlerRegistry registry, string pluginId)
        : this(registry, pluginId, null)
    {
    }

    public MessageRouter(
        HandlerRegistry registry,
        string pluginId,
        Func<IHandler, JsonNode?, object?>? invokeHandler)
    {
        _registry = registry ?? throw new ArgumentNullException(nameof(registry));
        _registry.Info.PluginId = pluginId;
        _invokeHandler = invokeHandler ?? ((handler, @params) => handler.Execute(@params));
    }

    public string HandleMessage(string jsonLine)
    {
        JsonObject request;
        try
        {
            request = JsonNode.Parse(jsonLine)?.AsObject()
                ?? throw new JsonException("Request must be a JSON object.");
        }
        catch (JsonException)
        {
            return ErrorResponse(null, -32700, "Parse error");
        }

        var idNode = request["id"];
        var id = ExtractId(idNode);
        if (!IsValidRequest(request, idNode))
        {
            return ErrorResponse(id, -32600, "Invalid Request");
        }

        var method = request["method"]!.GetValue<string>();
        var @params = request["params"];
        if (!_registry.TryGetHandler(method, out var handler))
        {
            return ErrorResponse(id, -32601, "Method not found");
        }

        try
        {
            var result = _invokeHandler(handler, @params);
            return SuccessResponse(id, result);
        }
        catch (RpcException ex)
        {
            return ErrorResponse(id, ex.Code, ex.Message, ex.Data);
        }
        catch
        {
            return ErrorResponse(id, -32603, "Internal error");
        }
    }

    private static bool IsValidRequest(JsonObject request, JsonNode? idNode)
    {
        if (request["jsonrpc"]?.GetValue<string>() != JsonRpcVersion)
        {
            return false;
        }

        if (!IsValidId(idNode))
        {
            return false;
        }

        if (request["method"] is not JsonValue methodValue ||
            !methodValue.TryGetValue<string>(out var method) ||
            string.IsNullOrWhiteSpace(method))
        {
            return false;
        }

        var paramsNode = request["params"];
        return paramsNode is null || paramsNode is JsonObject || paramsNode is JsonArray;
    }

    private static bool IsValidId(JsonNode? node)
    {
        if (node is null)
        {
            return false;
        }

        return ExtractId(node) is not null;
    }

    private static JsonNode? ExtractId(JsonNode? node)
    {
        if (node is null)
        {
            return null;
        }

        if (node is not JsonValue value)
        {
            return null;
        }

        if (value.TryGetValue<long>(out var longValue))
        {
            return JsonValue.Create(longValue);
        }

        if (value.TryGetValue<string>(out var stringValue))
        {
            return JsonValue.Create(stringValue);
        }

        return null;
    }

    private static string SuccessResponse(JsonNode? id, object? result)
    {
        var response = new JsonObject
        {
            ["jsonrpc"] = JsonRpcVersion,
            ["id"] = CloneNode(id),
            ["result"] = ToJsonNode(result)
        };
        return response.ToJsonString(JsonOptions);
    }

    private static string ErrorResponse(JsonNode? id, int code, string message, object? data = null)
    {
        var error = new JsonObject
        {
            ["code"] = code,
            ["message"] = message
        };
        if (data is not null)
        {
            error["data"] = ToJsonNode(data);
        }

        var response = new JsonObject
        {
            ["jsonrpc"] = JsonRpcVersion,
            ["id"] = CloneNode(id),
            ["error"] = error
        };
        return response.ToJsonString(JsonOptions);
    }

    private static JsonNode? ToJsonNode(object? value)
    {
        if (value is null)
        {
            return null;
        }

        return value is JsonNode node
            ? CloneNode(node)
            : JsonSerializer.SerializeToNode(value, JsonOptions);
    }

    private static JsonNode? CloneNode(JsonNode? node)
    {
        return node is null ? null : JsonNode.Parse(node.ToJsonString(JsonOptions));
    }
}
