using RhinoCli.Server.Reflection;
using Xunit;

namespace RhinoCli.Server.Tests;

public sealed class XmlDocLoaderTests
{
    private const string FixtureXml = """
        <?xml version="1.0"?>
        <doc>
          <assembly><name>FixtureAsm</name></assembly>
          <members>
            <member name="T:Sample.Widget">
              <summary>A widget for demos.</summary>
            </member>
            <member name="M:Sample.Widget.#ctor(System.Int32,System.String)">
              <summary>Construct with seed and label.</summary>
              <param name="seed">starting count</param>
              <param name="label">human-visible name</param>
            </member>
            <member name="M:Sample.Widget.Rotate(System.Double)">
              <summary>
                Rotates the widget by the given angle in degrees.
                Wrapping behavior is consistent with Math.IEEERemainder.
              </summary>
              <param name="degrees">positive = clockwise</param>
              <returns>the new total rotation</returns>
            </member>
            <member name="P:Sample.Widget.Count">
              <summary>Number of revolutions completed.</summary>
            </member>
            <member name="F:Sample.Widget.MaxCount">
              <summary>Hard upper bound on Count.</summary>
            </member>
            <member name="E:Sample.Widget.Spun">
              <summary>Raised when Rotate completes.</summary>
            </member>
          </members>
        </doc>
        """;

    [Fact]
    public void LookupReturnsParsedSummary()
    {
        var loader = XmlDocLoader.LoadFromString(FixtureXml);

        var docs = loader.Lookup("T:Sample.Widget");
        Assert.NotNull(docs);
        Assert.Equal("A widget for demos.", docs!.Summary);
    }

    [Fact]
    public void LookupNormalizesMultilineWhitespace()
    {
        var loader = XmlDocLoader.LoadFromString(FixtureXml);

        var docs = loader.Lookup("M:Sample.Widget.Rotate(System.Double)");
        Assert.NotNull(docs);
        Assert.Equal(
            "Rotates the widget by the given angle in degrees. Wrapping behavior is consistent with Math.IEEERemainder.",
            docs!.Summary);
        Assert.Equal("the new total rotation", docs.Returns);
    }

    [Fact]
    public void LookupCapturesPerParameterText()
    {
        var loader = XmlDocLoader.LoadFromString(FixtureXml);

        var docs = loader.Lookup("M:Sample.Widget.#ctor(System.Int32,System.String)");
        Assert.NotNull(docs);
        Assert.Equal("starting count", docs!.Params["seed"]);
        Assert.Equal("human-visible name", docs.Params["label"]);
    }

    [Fact]
    public void LookupReturnsNullForUnknownId()
    {
        var loader = XmlDocLoader.LoadFromString(FixtureXml);
        Assert.Null(loader.Lookup("T:Nope.Not.Found"));
    }

    [Fact]
    public void LookupCoversProperty_Field_Event()
    {
        var loader = XmlDocLoader.LoadFromString(FixtureXml);

        Assert.Equal("Number of revolutions completed.", loader.Lookup("P:Sample.Widget.Count")!.Summary);
        Assert.Equal("Hard upper bound on Count.", loader.Lookup("F:Sample.Widget.MaxCount")!.Summary);
        Assert.Equal("Raised when Rotate completes.", loader.Lookup("E:Sample.Widget.Spun")!.Summary);
    }

    [Fact]
    public void EmptyDocumentParsesWithoutError()
    {
        var loader = XmlDocLoader.LoadFromString(
            "<doc><assembly><name>x</name></assembly><members/></doc>");
        Assert.Equal(0, loader.Count);
    }
}
