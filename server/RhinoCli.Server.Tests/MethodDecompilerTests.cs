using RhinoCli.Server;
using RhinoCli.Server.Reflection;
using Xunit;

namespace RhinoCli.Server.Tests;

public sealed class MethodDecompilerTests
{
    [Fact]
    public void DecompileSimpleMethodReturnsNonEmptyCSharp()
    {
        var result = MethodDecompiler.Decompile(
            typeof(DecompileSample).FullName!,
            nameof(DecompileSample.AddOne),
            signatureFilter: null);

        Assert.Equal(typeof(DecompileSample).FullName, result.Type);
        Assert.Equal("AddOne", result.Method);
        Assert.False(string.IsNullOrEmpty(result.CSharp));
        Assert.Contains("AddOne", result.CSharp);
    }

    [Fact]
    public void DecompileAmbiguousOverloadThrowsWithSignatures()
    {
        var ex = Assert.Throws<RpcException>(() => MethodDecompiler.Decompile(
            typeof(DecompileSample).FullName!,
            nameof(DecompileSample.Combine),
            signatureFilter: null));

        Assert.Equal(-32602, ex.Code);
        var json = System.Text.Json.JsonSerializer.Serialize(ex.Data);
        Assert.Contains("ambiguous_overload", json);
        Assert.Contains("available", json);
    }

    [Fact]
    public void DecompileWithSignatureDisambiguatesOverload()
    {
        var result = MethodDecompiler.Decompile(
            typeof(DecompileSample).FullName!,
            nameof(DecompileSample.Combine),
            signatureFilter: "System.Int32,System.Int32");

        Assert.False(string.IsNullOrEmpty(result.CSharp));
        Assert.Contains("Combine", result.CSharp);
    }

    [Fact]
    public void DecompileSignatureShortNameAlsoMatches()
    {
        var result = MethodDecompiler.Decompile(
            typeof(DecompileSample).FullName!,
            nameof(DecompileSample.Combine),
            signatureFilter: "Int32,Int32");

        Assert.False(string.IsNullOrEmpty(result.CSharp));
    }

    [Fact]
    public void DecompileUnknownTypeThrows()
    {
        var ex = Assert.Throws<RpcException>(() => MethodDecompiler.Decompile(
            "No.Such.Decompile.Target",
            "X",
            signatureFilter: null));

        Assert.Equal(-32602, ex.Code);
        var json = System.Text.Json.JsonSerializer.Serialize(ex.Data);
        Assert.Contains("type_not_found", json);
    }

    [Fact]
    public void DecompileUnknownMethodThrows()
    {
        var ex = Assert.Throws<RpcException>(() => MethodDecompiler.Decompile(
            typeof(DecompileSample).FullName!,
            "NoSuchMethodName",
            signatureFilter: null));

        Assert.Equal(-32602, ex.Code);
        var json = System.Text.Json.JsonSerializer.Serialize(ex.Data);
        Assert.Contains("method_not_found", json);
    }

    [Fact]
    public void DecompileBadSignatureThrows()
    {
        var ex = Assert.Throws<RpcException>(() => MethodDecompiler.Decompile(
            typeof(DecompileSample).FullName!,
            nameof(DecompileSample.Combine),
            signatureFilter: "System.String"));

        Assert.Equal(-32602, ex.Code);
        var json = System.Text.Json.JsonSerializer.Serialize(ex.Data);
        Assert.Contains("no_overload_matches", json);
    }

    [Fact]
    public void DecompileConstructorViaCtorAlias()
    {
        var result = MethodDecompiler.Decompile(
            typeof(DecompileSample).FullName!,
            ".ctor",
            signatureFilter: "System.Int32");

        Assert.Equal(".ctor", result.Method);
        Assert.False(string.IsNullOrEmpty(result.CSharp));
    }

    [Fact]
    public void DecompileEmptyTypeThrows()
    {
        var ex = Assert.Throws<RpcException>(() => MethodDecompiler.Decompile("", "foo", null));
        Assert.Equal(-32602, ex.Code);
    }

    [Fact]
    public void DecompileEmptyMethodThrows()
    {
        var ex = Assert.Throws<RpcException>(() => MethodDecompiler.Decompile("System.String", "", null));
        Assert.Equal(-32602, ex.Code);
    }

    public sealed class DecompileSample
    {
        public DecompileSample(int seed) { Seed = seed; }

        public int Seed { get; }

        public int AddOne(int x) => x + 1;

        public int Combine(int a, int b) => a + b;

        public string Combine(string a, string b) => a + b;
    }
}
