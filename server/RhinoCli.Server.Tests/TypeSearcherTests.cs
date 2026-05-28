using System.Reflection;
using RhinoCli.Server;
using RhinoCli.Server.Reflection;
using Xunit;

namespace RhinoCli.Server.Tests;

public sealed class TypeSearcherTests
{
    [Fact]
    public void SearchFindsTypeByNameSubstring()
    {
        var result = TypeSearcher.Search(
            new SearchOptions("SearchFixture", SearchScope.All, AssemblyOf<TypeSearcherTests>(), 50),
            new[] { typeof(TypeSearcherTests).Assembly });

        Assert.Contains(result.Matches, m =>
            m.Kind == "type" &&
            m.FullName == typeof(SearchFixture).FullName);
    }

    [Fact]
    public void SearchFindsMemberByName()
    {
        var result = TypeSearcher.Search(
            new SearchOptions("UniqueMethodNameForSearch", SearchScope.All, AssemblyOf<TypeSearcherTests>(), 50),
            new[] { typeof(TypeSearcherTests).Assembly });

        Assert.Contains(result.Matches, m =>
            m.Kind == "method" &&
            m.Member == "UniqueMethodNameForSearch" &&
            m.FullName == typeof(SearchFixture).FullName);
    }

    [Fact]
    public void SearchScopeTypesExcludesMembers()
    {
        var result = TypeSearcher.Search(
            new SearchOptions("UniqueMethodNameForSearch", SearchScope.Types, AssemblyOf<TypeSearcherTests>(), 50),
            new[] { typeof(TypeSearcherTests).Assembly });

        Assert.DoesNotContain(result.Matches, m => m.Member == "UniqueMethodNameForSearch");
    }

    [Fact]
    public void SearchScopeMembersExcludesTypes()
    {
        var result = TypeSearcher.Search(
            new SearchOptions("SearchFixture", SearchScope.Members, AssemblyOf<TypeSearcherTests>(), 50),
            new[] { typeof(TypeSearcherTests).Assembly });

        Assert.DoesNotContain(result.Matches, m => m.Kind == "type" && m.Member is null);
    }

    [Fact]
    public void SearchAppliesLimitAndTruncated()
    {
        var result = TypeSearcher.Search(
            new SearchOptions("e", SearchScope.All, AssemblyOf<TypeSearcherTests>(), 2),
            new[] { typeof(TypeSearcherTests).Assembly });

        Assert.True(result.Matches.Count <= 2);
        if (result.Matches.Count == 2)
        {
            Assert.True(result.Truncated);
        }
    }

    [Fact]
    public void SearchEmptyPatternThrows()
    {
        var ex = Assert.Throws<RpcException>(
            () => TypeSearcher.Search(new SearchOptions("", SearchScope.All, null, 50)));
        Assert.Equal(-32602, ex.Code);
    }

    [Fact]
    public void SearchIsCaseInsensitive()
    {
        var result = TypeSearcher.Search(
            new SearchOptions("uniquemethodnameforsearch", SearchScope.All, AssemblyOf<TypeSearcherTests>(), 50),
            new[] { typeof(TypeSearcherTests).Assembly });

        Assert.Contains(result.Matches, m => m.Member == "UniqueMethodNameForSearch");
    }

    [Fact]
    public void ParseScopeAcceptsKnownValues()
    {
        Assert.Equal(SearchScope.All, TypeSearcher.ParseScope(null));
        Assert.Equal(SearchScope.All, TypeSearcher.ParseScope(""));
        Assert.Equal(SearchScope.All, TypeSearcher.ParseScope("all"));
        Assert.Equal(SearchScope.Types, TypeSearcher.ParseScope("types"));
        Assert.Equal(SearchScope.Members, TypeSearcher.ParseScope("members"));
    }

    [Fact]
    public void ParseScopeRejectsUnknown()
    {
        var ex = Assert.Throws<RpcException>(() => TypeSearcher.ParseScope("BOTH"));
        Assert.Equal(-32602, ex.Code);
    }

    private static string AssemblyOf<T>() => typeof(T).Assembly.GetName().Name!;

    public sealed class SearchFixture
    {
        public int UniqueMethodNameForSearch() => 42;

        public string AnotherSearchableMember { get; set; } = "";
    }
}
