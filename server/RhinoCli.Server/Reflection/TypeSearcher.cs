using System.Reflection;

namespace RhinoCli.Server.Reflection;

public enum SearchScope
{
    All,
    Types,
    Members,
}

public sealed record SearchOptions(
    string Pattern,
    SearchScope Scope = SearchScope.All,
    string? AssemblyFilter = null,
    int Limit = 50);

public sealed class SearchMatch
{
    public string Kind { get; init; } = "";          // type | method | property | field | event | constructor
    public string FullName { get; init; } = "";      // type FQN (for type kinds) or DeclaringType FQN (for members)
    public string? Member { get; init; }              // member name when Kind != "type"
    public string Assembly { get; init; } = "";
}

public sealed class SearchResult
{
    public IReadOnlyList<SearchMatch> Matches { get; init; } = Array.Empty<SearchMatch>();
    public bool Truncated { get; init; }
}

public static class TypeSearcher
{
    private static readonly string[] DefaultAssemblyPrefixes =
    {
        "Rhino",
        "RhinoCommon",
        "RhinoCli",
    };

    public static SearchScope ParseScope(string? value)
    {
        return (value ?? "").Trim().ToLowerInvariant() switch
        {
            "" or "all" => SearchScope.All,
            "types" => SearchScope.Types,
            "members" => SearchScope.Members,
            _ => throw new RpcException(
                -32602,
                "Invalid params",
                new { field = "scope", expected = "one of: all, types, members", actual = value }),
        };
    }

    public static SearchResult Search(SearchOptions options, IEnumerable<Assembly>? assemblies = null)
    {
        if (string.IsNullOrWhiteSpace(options.Pattern))
        {
            throw new RpcException(
                -32602,
                "Invalid params",
                new { field = "pattern", expected = "non-empty substring" });
        }

        var limit = options.Limit <= 0 ? 50 : options.Limit;
        var pattern = options.Pattern;
        var matches = new List<SearchMatch>(limit);
        var truncated = false;

        var pool = assemblies ?? AppDomain.CurrentDomain.GetAssemblies();
        foreach (var assembly in FilterAssemblies(pool, options.AssemblyFilter))
        {
            Type[] types;
            try
            {
                types = assembly.GetTypes();
            }
            catch (ReflectionTypeLoadException ex)
            {
                types = ex.Types.Where(t => t is not null).Cast<Type>().ToArray();
            }
            catch
            {
                continue;
            }

            foreach (var type in types)
            {
                if (matches.Count >= limit)
                {
                    truncated = true;
                    break;
                }

                if (type is null || !type.IsVisible)
                {
                    continue;
                }

                if (options.Scope is SearchScope.All or SearchScope.Types
                    && ContainsIgnoreCase(type.Name, pattern))
                {
                    matches.Add(new SearchMatch
                    {
                        Kind = "type",
                        FullName = type.FullName ?? type.Name,
                        Assembly = assembly.GetName().Name ?? "",
                    });
                    if (matches.Count >= limit) { truncated = true; break; }
                }

                if (options.Scope is SearchScope.All or SearchScope.Members)
                {
                    AppendMemberMatches(matches, type, assembly, pattern, limit, ref truncated);
                    if (matches.Count >= limit) break;
                }
            }

            if (matches.Count >= limit && truncated)
            {
                break;
            }
        }

        return new SearchResult
        {
            Matches = matches,
            Truncated = truncated,
        };
    }

    private static IEnumerable<Assembly> FilterAssemblies(IEnumerable<Assembly> pool, string? filter)
    {
        if (!string.IsNullOrEmpty(filter))
        {
            var f = filter;
            return pool.Where(a => string.Equals(a.GetName().Name, f, StringComparison.OrdinalIgnoreCase));
        }

        return pool.Where(a =>
        {
            var name = a.GetName().Name ?? "";
            foreach (var prefix in DefaultAssemblyPrefixes)
            {
                if (name.StartsWith(prefix, StringComparison.OrdinalIgnoreCase))
                {
                    return true;
                }
            }
            return false;
        });
    }

    private static void AppendMemberMatches(
        List<SearchMatch> sink,
        Type type,
        Assembly assembly,
        string pattern,
        int limit,
        ref bool truncated)
    {
        const BindingFlags flags = BindingFlags.Public | BindingFlags.Instance | BindingFlags.Static | BindingFlags.DeclaredOnly;

        var assemblyName = assembly.GetName().Name ?? "";
        var typeFullName = type.FullName ?? type.Name;

        foreach (var member in type.GetMembers(flags))
        {
            if (sink.Count >= limit) { truncated = true; return; }

            // Skip property accessors and event add/remove
            if (member is MethodBase mb && mb.IsSpecialName)
            {
                continue;
            }

            if (!ContainsIgnoreCase(member.Name, pattern))
            {
                continue;
            }

            var kind = member switch
            {
                ConstructorInfo => "constructor",
                MethodInfo => "method",
                PropertyInfo => "property",
                FieldInfo => "field",
                EventInfo => "event",
                _ => "member",
            };

            sink.Add(new SearchMatch
            {
                Kind = kind,
                FullName = typeFullName,
                Member = member.Name,
                Assembly = assemblyName,
            });
        }
    }

    private static bool ContainsIgnoreCase(string source, string pattern)
    {
        if (string.IsNullOrEmpty(source) || string.IsNullOrEmpty(pattern))
        {
            return false;
        }
        return source.IndexOf(pattern, StringComparison.OrdinalIgnoreCase) >= 0;
    }
}
