using System.Reflection;
using System.Text.Json;
using System.Text.Json.Nodes;
using RhinoCli.Server;
using RhinoCli.Server.Reflection;
using Xunit;

namespace RhinoCli.Server.Tests;

public sealed class TypeInspectorTests
{
    [Fact]
    public void InspectSystemStringReturnsExpectedShape()
    {
        var node = InspectAsJson("System.String", InspectBinding.Public, includeInherited: false);

        Assert.Equal("System.String", node["full_name"]!.GetValue<string>());
        Assert.Equal("class", node["kind"]!.GetValue<string>());
        Assert.False(node["is_abstract"]!.GetValue<bool>());
        Assert.True(node["is_sealed"]!.GetValue<bool>());

        var properties = node["properties"]!.AsArray();
        Assert.Contains(properties, p => p!["name"]!.GetValue<string>() == "Length");

        var methods = node["methods"]!.AsArray();
        Assert.Contains(methods, m => m!["name"]!.GetValue<string>() == "Substring");
    }

    [Fact]
    public void InspectGroupsMethodOverloads()
    {
        var node = InspectAsJson("System.String", InspectBinding.Public, includeInherited: false);

        var substring = node["methods"]!
            .AsArray()
            .First(m => m!["name"]!.GetValue<string>() == "Substring")!;

        var overloads = substring["overloads"]!.AsArray();
        Assert.True(overloads.Count >= 2, "Substring should expose multiple overloads");
        foreach (var overload in overloads)
        {
            Assert.NotNull(overload!["return_type"]);
            Assert.NotNull(overload["params"]);
        }
    }

    [Fact]
    public void InspectIncludeInheritedAffectsMemberCount()
    {
        var declaredOnly = InspectAsJson(typeof(LocalSample).FullName!, InspectBinding.Public, includeInherited: false);
        var inherited = InspectAsJson(typeof(LocalSample).FullName!, InspectBinding.Public, includeInherited: true);

        var declaredMethods = declaredOnly["methods"]!.AsArray().Count;
        var inheritedMethods = inherited["methods"]!.AsArray().Count;
        Assert.True(
            inheritedMethods > declaredMethods,
            $"include_inherited=true should report more methods (declared={declaredMethods}, inherited={inheritedMethods})");
    }

    [Fact]
    public void InspectStructTypeReportsStructKind()
    {
        var node = InspectAsJson(typeof(LocalPoint).FullName!, InspectBinding.Public, includeInherited: false);

        Assert.Equal("struct", node["kind"]!.GetValue<string>());

        var constructors = node["constructors"]!.AsArray();
        Assert.Contains(
            constructors,
            c => c!["params"]!.AsArray().Count == 2);
    }

    [Fact]
    public void InspectEnumTypeReportsEnumKind()
    {
        var node = InspectAsJson(typeof(LocalKind).FullName!, InspectBinding.Public, includeInherited: false);

        Assert.Equal("enum", node["kind"]!.GetValue<string>());

        var fields = node["fields"]!.AsArray();
        Assert.Contains(fields, f => f!["name"]!.GetValue<string>() == "Alpha");
        Assert.Contains(fields, f => f!["name"]!.GetValue<string>() == "Beta");
    }

    [Fact]
    public void InspectMissingTypeThrowsInvalidParams()
    {
        var ex = Assert.Throws<RpcException>(
            () => TypeInspector.Inspect("NoSuch.Type.Exists", new InspectOptions()));

        Assert.Equal(-32602, ex.Code);
        var data = JsonSerializer.SerializeToNode(ex.Data)!;
        Assert.Equal("type_not_found", data["reason"]!.GetValue<string>());
        Assert.Equal("NoSuch.Type.Exists", data["value"]!.GetValue<string>());
    }

    [Fact]
    public void ParseBindingAcceptsKnownValues()
    {
        Assert.Equal(InspectBinding.Public, TypeInspector.ParseBinding(null));
        Assert.Equal(InspectBinding.Public, TypeInspector.ParseBinding(""));
        Assert.Equal(InspectBinding.Public, TypeInspector.ParseBinding("public"));
        Assert.Equal(InspectBinding.PublicInstance, TypeInspector.ParseBinding("public_instance"));
        Assert.Equal(InspectBinding.PublicStatic, TypeInspector.ParseBinding("public_static"));
        Assert.Equal(InspectBinding.NonPublic, TypeInspector.ParseBinding("non_public"));
        Assert.Equal(InspectBinding.All, TypeInspector.ParseBinding("all"));
    }

    [Fact]
    public void ParseBindingRejectsUnknownValues()
    {
        var ex = Assert.Throws<RpcException>(() => TypeInspector.ParseBinding("PUBLIC_PRIVATE"));
        Assert.Equal(-32602, ex.Code);
    }

    [Fact]
    public void FormatTypeHandlesGenericsArraysAndByRef()
    {
        Assert.Equal("System.Int32", TypeInspector.FormatType(typeof(int)));
        Assert.Equal("System.Int32[]", TypeInspector.FormatType(typeof(int[])));
        Assert.Equal(
            "System.Collections.Generic.List<System.String>",
            TypeInspector.FormatType(typeof(List<string>)));
        Assert.Equal(
            "System.Collections.Generic.Dictionary<System.String, System.Int32>",
            TypeInspector.FormatType(typeof(Dictionary<string, int>)));

        var byRef = typeof(int).MakeByRefType();
        Assert.Equal("System.Int32", TypeInspector.FormatType(byRef));
    }

    [Fact]
    public void InspectAttachesSummaryFromXmlDocLoader()
    {
        var xml = """
            <?xml version="1.0"?>
            <doc>
              <assembly><name>Fixture</name></assembly>
              <members>
                <member name="T:RhinoCli.Server.Tests.TypeInspectorTests.AnnotatedSample">
                  <summary>Annotated sample for inspector docs test.</summary>
                </member>
                <member name="M:RhinoCli.Server.Tests.TypeInspectorTests.AnnotatedSample.#ctor(System.Int32)">
                  <summary>Construct with seed.</summary>
                  <param name="seed">starting count</param>
                </member>
                <member name="M:RhinoCli.Server.Tests.TypeInspectorTests.AnnotatedSample.Compute(System.Int32)">
                  <summary>Compute the next value.</summary>
                  <param name="step">increment</param>
                  <returns>new value</returns>
                </member>
                <member name="P:RhinoCli.Server.Tests.TypeInspectorTests.AnnotatedSample.Counter">
                  <summary>Current counter value.</summary>
                </member>
              </members>
            </doc>
            """;
        var docs = XmlDocLoader.LoadFromString(xml);

        var result = TypeInspector.Inspect(typeof(AnnotatedSample), new InspectOptions(), docs);
        var json = JsonSerializer.Serialize(result);
        var node = JsonNode.Parse(json)!.AsObject();

        Assert.Equal("Annotated sample for inspector docs test.", node["summary"]!.GetValue<string>());

        var ctor = node["constructors"]!.AsArray().First()!;
        Assert.Equal("Construct with seed.", ctor["summary"]!.GetValue<string>());
        Assert.Equal("starting count", ctor["params"]!.AsArray().First()!["summary"]!.GetValue<string>());

        var compute = node["methods"]!
            .AsArray()
            .First(m => m!["name"]!.GetValue<string>() == "Compute")!;
        var firstOverload = compute["overloads"]!.AsArray().First()!;
        Assert.Equal("Compute the next value.", firstOverload["summary"]!.GetValue<string>());
        Assert.Equal("new value", firstOverload["returns"]!.GetValue<string>());
        Assert.Equal("increment", firstOverload["params"]!.AsArray().First()!["summary"]!.GetValue<string>());

        var counter = node["properties"]!
            .AsArray()
            .First(p => p!["name"]!.GetValue<string>() == "Counter")!;
        Assert.Equal("Current counter value.", counter["summary"]!.GetValue<string>());
    }

    [Fact]
    public void InspectReturnsEmptySummaryWhenXmlMissing()
    {
        // System.String has no XML doc beside its assembly in test environment by default.
        var node = InspectAsJson("System.String", InspectBinding.Public, includeInherited: false);
        // summary field should exist but may be empty string.
        Assert.True(node.ContainsKey("summary"));
        Assert.NotNull(node["summary"]);
    }

    [Fact]
    public void ResolveTypeFallsBackThroughLoadedAssemblies()
    {
        // System.String is in mscorlib/System.Private.CoreLib, which is loaded.
        Assert.NotNull(TypeInspector.ResolveType("System.String"));
        // Our local type is in the test assembly, which is also loaded.
        Assert.NotNull(TypeInspector.ResolveType(typeof(LocalPoint).FullName!));
        Assert.Null(TypeInspector.ResolveType("Definitely.Not.A.Real.Type"));
    }

    private static JsonObject InspectAsJson(string name, InspectBinding binding, bool includeInherited)
    {
        var result = TypeInspector.Inspect(name, new InspectOptions(binding, includeInherited));
        var json = JsonSerializer.Serialize(result);
        return JsonNode.Parse(json)!.AsObject();
    }

    public class LocalSampleBase
    {
        public int InheritedProperty { get; set; }

        public string InheritedMethod() => "base";
    }

    public sealed class LocalSample : LocalSampleBase
    {
        public string DeclaredProperty { get; set; } = "";

        public int DeclaredMethod(int x) => x;

        public int DeclaredMethod(int x, int y) => x + y;
    }

    public readonly struct LocalPoint
    {
        public LocalPoint(double x, double y)
        {
            X = x;
            Y = y;
        }

        public double X { get; }
        public double Y { get; }
    }

    public enum LocalKind
    {
        Alpha,
        Beta,
        Gamma,
    }

    public sealed class AnnotatedSample
    {
        public AnnotatedSample(int seed)
        {
            Counter = seed;
        }

        public int Counter { get; set; }

        public int Compute(int step) => Counter + step;
    }
}
