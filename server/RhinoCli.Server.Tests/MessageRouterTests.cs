using System.Text.Json.Nodes;
using RhinoCli.Server;
using Xunit;

namespace RhinoCli.Server.Tests;

public sealed class MessageRouterTests
{
    [Fact]
    public void PingReturnsSuccessResponse()
    {
        var router = CreateRouter();

        var response = Handle(router, """{"jsonrpc":"2.0","id":1,"method":"system.ping","params":null}""");

        Assert.Equal("2.0", response["jsonrpc"]!.GetValue<string>());
        Assert.Equal(1, response["id"]!.GetValue<int>());
        Assert.True(response["result"]!["pong"]!.GetValue<bool>());
        Assert.Equal("TestPlugin", response["result"]!["server"]!.GetValue<string>());
    }

    [Fact]
    public void MethodNotFoundReturnsJsonRpcError()
    {
        var router = CreateRouter();

        var response = Handle(router, """{"jsonrpc":"2.0","id":2,"method":"missing.method","params":null}""");

        Assert.Equal(-32601, response["error"]!["code"]!.GetValue<int>());
        Assert.Equal("Method not found", response["error"]!["message"]!.GetValue<string>());
    }

    [Fact]
    public void ParseErrorReturnsNullId()
    {
        var router = CreateRouter();

        var response = Handle(router, """{"jsonrpc":"2.0","id":3,"meth""");

        Assert.Null(response["id"]);
        Assert.Equal(-32700, response["error"]!["code"]!.GetValue<int>());
    }

    [Fact]
    public void MissingMethodReturnsInvalidRequest()
    {
        var router = CreateRouter();

        var response = Handle(router, """{"jsonrpc":"2.0","id":4}""");

        Assert.Equal(4, response["id"]!.GetValue<int>());
        Assert.Equal(-32600, response["error"]!["code"]!.GetValue<int>());
    }

    [Fact]
    public void RpcExceptionIsReturnedAsHandlerError()
    {
        var registry = CreateRegistry();
        registry.Register("test.invalid_params", new ThrowRpcHandler());
        var router = new MessageRouter(registry, "TestPlugin");

        var response = Handle(router, """{"jsonrpc":"2.0","id":5,"method":"test.invalid_params","params":{}}""");

        Assert.Equal(-32602, response["error"]!["code"]!.GetValue<int>());
        Assert.Equal("Invalid params", response["error"]!["message"]!.GetValue<string>());
        Assert.Equal("count", response["error"]!["data"]!["field"]!.GetValue<string>());
    }

    [Fact]
    public void GenericExceptionReturnsInternalError()
    {
        var registry = CreateRegistry();
        registry.Register("test.boom", new ThrowGenericHandler());
        var router = new MessageRouter(registry, "TestPlugin");

        var response = Handle(router, """{"jsonrpc":"2.0","id":6,"method":"test.boom","params":{}}""");

        Assert.Equal(-32603, response["error"]!["code"]!.GetValue<int>());
        Assert.Equal("Internal error", response["error"]!["message"]!.GetValue<string>());
    }

    [Fact]
    public void BuiltInVersionReturnsProtocolAndPlugin()
    {
        var router = CreateRouter();

        var response = Handle(router, """{"jsonrpc":"2.0","id":7,"method":"system.version"}""");

        Assert.Equal("jsonrpc-2.0", response["result"]!["protocol"]!.GetValue<string>());
        Assert.Equal("0.1.0", response["result"]!["server"]!.GetValue<string>());
        Assert.Equal("TestPlugin", response["result"]!["plugin"]!.GetValue<string>());
    }

    [Fact]
    public void BuiltInListMethodsIncludesSystemAndCustomMethods()
    {
        var registry = CreateRegistry();
        registry.Register("test.echo", new EchoHandler(), new HandlerMetadata
        {
            Description = "Echo test params.",
            ParamsSchema = "any",
            ResultSchema = "same as params",
            Examples = new[] { "rhino-cli call test.echo '{\"ok\":true}'" },
            Category = "test"
        });
        var router = new MessageRouter(registry, "TestPlugin");

        var response = Handle(router, """{"jsonrpc":"2.0","id":8,"method":"rpc.list_methods"}""");
        var methods = response["result"]!["methods"]!.AsArray().Select(node => node!.GetValue<string>()).ToArray();

        Assert.Contains("system.ping", methods);
        Assert.Contains("system.version", methods);
        Assert.Contains("rpc.capabilities", methods);
        Assert.Contains("rpc.list_methods", methods);
        Assert.Contains("rpc.list_plugins", methods);
        Assert.Contains("test.echo", methods);
    }

    [Fact]
    public void BuiltInCapabilitiesReturnsMethodMetadata()
    {
        var registry = CreateRegistry();
        registry.Register("test.echo", new EchoHandler(), new HandlerMetadata
        {
            Description = "Echo test params.",
            ParamsSchema = "any",
            ResultSchema = "same as params",
            Examples = new[] { "rhino-cli call test.echo '{\"ok\":true}'" },
            Category = "test"
        });
        var router = new MessageRouter(registry, "TestPlugin");

        var response = Handle(router, """{"jsonrpc":"2.0","id":9,"method":"rpc.capabilities","params":{"method":"test.echo"}}""");
        var method = response["result"]!["method"]!;

        Assert.Equal("TestPlugin", response["result"]!["server"]!["plugin_id"]!.GetValue<string>());
        Assert.Equal("test.echo", method["method"]!.GetValue<string>());
        Assert.Equal("Echo test params.", method["description"]!.GetValue<string>());
        Assert.Equal("any", method["paramsSchema"]!.GetValue<string>());
        Assert.Equal("same as params", method["resultSchema"]!.GetValue<string>());
        Assert.Equal("test", method["category"]!.GetValue<string>());
        Assert.Equal("rhino-cli call test.echo '{\"ok\":true}'", method["examples"]![0]!.GetValue<string>());
    }

    [Fact]
    public void RegisterUsesHandlerMetadataAttributeWhenExplicitMetadataIsNotProvided()
    {
        var registry = CreateRegistry();
        registry.Register("test.attribute", new AttributeMetadataHandler());
        var router = new MessageRouter(registry, "TestPlugin");

        var response = Handle(router, """{"jsonrpc":"2.0","id":10,"method":"rpc.capabilities","params":{"method":"test.attribute"}}""");
        var method = response["result"]!["method"]!;

        Assert.Equal("Metadata from attribute.", method["description"]!.GetValue<string>());
        Assert.Equal("{ value: number }", method["paramsSchema"]!.GetValue<string>());
        Assert.Equal("attribute", method["category"]!.GetValue<string>());
    }

    [Fact]
    public void BuiltInListPluginsReturnsSinglePlugin()
    {
        var router = CreateRouter(port: 50099);

        var response = Handle(router, """{"jsonrpc":"2.0","id":11,"method":"rpc.list_plugins"}""");
        var plugin = response["result"]!["plugins"]![0]!;

        Assert.Equal("TestPlugin", plugin["id"]!.GetValue<string>());
        Assert.Equal(50099, plugin["port"]!.GetValue<int>());
    }

    [Fact]
    public void InvokeDelegateCanWrapHandlerExecution()
    {
        var registry = CreateRegistry();
        registry.Register("test.echo", new EchoHandler());
        var invoked = false;
        var router = new MessageRouter(registry, "TestPlugin", (handler, @params) =>
        {
            invoked = true;
            return handler.Execute(@params);
        });

        var response = Handle(router, """{"jsonrpc":"2.0","id":12,"method":"test.echo","params":{"ok":true}}""");

        Assert.True(invoked);
        Assert.True(response["result"]!["ok"]!.GetValue<bool>());
    }

    private static MessageRouter CreateRouter(int port = 50061)
    {
        return new MessageRouter(CreateRegistry(port), "TestPlugin");
    }

    private static HandlerRegistry CreateRegistry(int port = 50061)
    {
        return new HandlerRegistry("TestPlugin", port);
    }

    private static JsonObject Handle(MessageRouter router, string message)
    {
        return JsonNode.Parse(router.HandleMessage(message))!.AsObject();
    }

    private sealed class EchoHandler : IHandler
    {
        public object? Execute(JsonNode? @params) => @params;
    }

    private sealed class ThrowRpcHandler : IHandler
    {
        public object? Execute(JsonNode? @params)
        {
            throw new RpcException(-32602, "Invalid params", new { field = "count" });
        }
    }

    private sealed class ThrowGenericHandler : IHandler
    {
        public object? Execute(JsonNode? @params)
        {
            throw new InvalidOperationException("boom");
        }
    }

    [HandlerMetadataAttribute(
        "Metadata from attribute.",
        ParamsSchema = "{ value: number }",
        ResultSchema = "{ ok: boolean }",
        Category = "attribute")]
    private sealed class AttributeMetadataHandler : IHandler
    {
        public object? Execute(JsonNode? @params) => new { ok = true };
    }
}
