using System.Reflection;
using RhinoCli.Server.Reflection;
using Xunit;

namespace RhinoCli.Server.Tests;

public sealed class DocCommentIdTests
{
    [Fact]
    public void ForTypeReturnsTPrefixedName()
    {
        Assert.Equal("T:System.String", DocCommentId.ForType(typeof(string)));
    }

    [Fact]
    public void ForGenericTypeDefinitionKeepsBacktickArity()
    {
        Assert.Equal(
            "T:System.Collections.Generic.List`1",
            DocCommentId.ForType(typeof(List<>)));
    }

    [Fact]
    public void ForPropertyUsesPPrefix()
    {
        var prop = typeof(string).GetProperty("Length")!;
        Assert.Equal("P:System.String.Length", DocCommentId.ForProperty(prop));
    }

    [Fact]
    public void ForFieldUsesFPrefix()
    {
        var field = typeof(string).GetField("Empty")!;
        Assert.Equal("F:System.String.Empty", DocCommentId.ForField(field));
    }

    [Fact]
    public void ForConstructorUsesCtorMarker()
    {
        var ctor = typeof(string).GetConstructor(new[] { typeof(char[]) })!;
        Assert.Equal("M:System.String.#ctor(System.Char[])", DocCommentId.ForMethod(ctor));
    }

    [Fact]
    public void ForMethodEncodesParameterTypes()
    {
        var method = typeof(string).GetMethod(
            "Substring",
            new[] { typeof(int), typeof(int) })!;
        Assert.Equal("M:System.String.Substring(System.Int32,System.Int32)", DocCommentId.ForMethod(method));
    }

    [Fact]
    public void ForMethodHandlesByRefParameter()
    {
        var method = typeof(SampleHost).GetMethod(nameof(SampleHost.TryParse))!;
        Assert.Equal(
            "M:RhinoCli.Server.Tests.DocCommentIdTests.SampleHost.TryParse(System.String,System.Int32@)",
            DocCommentId.ForMethod(method));
    }

    [Fact]
    public void ForGenericMethodAppendsDoubleBacktickArity()
    {
        var method = typeof(SampleHost).GetMethod(nameof(SampleHost.MakePair))!;
        // Method `T MakePair<T>(T value)` -> ``0 for the type parameter
        Assert.Equal(
            "M:RhinoCli.Server.Tests.DocCommentIdTests.SampleHost.MakePair``1(``0)",
            DocCommentId.ForMethod(method));
    }

    [Fact]
    public void ForMethodEncodesGenericParameter()
    {
        var method = typeof(SampleHost).GetMethod(nameof(SampleHost.TakeList))!;
        Assert.Equal(
            "M:RhinoCli.Server.Tests.DocCommentIdTests.SampleHost.TakeList(System.Collections.Generic.List{System.Int32})",
            DocCommentId.ForMethod(method));
    }

    [Fact]
    public void ForEventUsesEPrefix()
    {
        var evt = typeof(SampleHost).GetEvent(nameof(SampleHost.Pinged))!;
        Assert.Equal(
            "E:RhinoCli.Server.Tests.DocCommentIdTests.SampleHost.Pinged",
            DocCommentId.ForEvent(evt));
    }

    public sealed class SampleHost
    {
        public event EventHandler? Pinged;

        public bool TryParse(string s, out int value)
        {
            value = 0;
            Pinged?.Invoke(this, EventArgs.Empty);
            return false;
        }

        public T MakePair<T>(T value) => value;

        public void TakeList(List<int> values)
        {
            _ = values;
        }
    }
}
