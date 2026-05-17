using System.Collections.Concurrent;
using System.Reflection;
using System.Reflection.Metadata;
using System.Reflection.PortableExecutable;
using ICSharpCode.Decompiler;
using ICSharpCode.Decompiler.CSharp;
using ICSharpCode.Decompiler.Metadata;
using ICSharpCode.Decompiler.TypeSystem;

namespace RhinoCli.Server.Reflection;

public sealed class DecompileResult
{
    public string Type { get; init; } = "";
    public string Method { get; init; } = "";
    public string Signature { get; init; } = "";
    public string CSharp { get; init; } = "";
    public string Summary { get; init; } = "";
}

public static class MethodDecompiler
{
    private static readonly ConcurrentDictionary<string, CSharpDecompiler> Cache = new();

    public static DecompileResult Decompile(string typeFullName, string methodName, string? signatureFilter)
    {
        if (string.IsNullOrWhiteSpace(typeFullName))
        {
            throw new RpcException(
                -32602,
                "Invalid params",
                new { field = "type", expected = "fully qualified type name" });
        }

        if (string.IsNullOrWhiteSpace(methodName))
        {
            throw new RpcException(
                -32602,
                "Invalid params",
                new { field = "method", expected = "non-empty method name (use '.ctor' for constructors)" });
        }

        var reflectionType = TypeInspector.ResolveType(typeFullName)
            ?? throw new RpcException(
                -32602,
                "Invalid params",
                new
                {
                    field = "type",
                    reason = "type_not_found",
                    value = typeFullName,
                    hint = "Use rhino.search_types to discover the FQN."
                });

        var assemblyPath = TryGetAssemblyPath(reflectionType.Assembly);
        if (assemblyPath is null)
        {
            throw new RpcException(
                -32603,
                "Internal error",
                new { reason = "assembly_path_unknown", type = typeFullName });
        }

        if (!Cache.TryGetValue(assemblyPath, out var decompiler))
        {
            try
            {
                decompiler = CreateDecompiler(assemblyPath);
                Cache[assemblyPath] = decompiler;
            }
            catch (Exception ex)
            {
                throw new RpcException(
                    -32603,
                    "Internal error",
                    new
                    {
                        reason = "decompiler_init_failed",
                        assembly = assemblyPath,
                        message = ex.Message,
                        exception = ex.GetType().FullName,
                    });
            }
        }

        var typeDef = decompiler.TypeSystem
            .FindType(new FullTypeName(typeFullName))
            .GetDefinition()
            ?? throw new RpcException(
                -32602,
                "Invalid params",
                new { field = "type", reason = "type_not_in_assembly", value = typeFullName });

        var isCtor = methodName == ".ctor" || methodName == "#ctor";
        var candidates = isCtor
            ? typeDef.GetConstructors().ToList()
            : typeDef.Methods.Where(m => m.Name == methodName).ToList();

        if (candidates.Count == 0)
        {
            throw new RpcException(
                -32602,
                "Invalid params",
                new { field = "method", reason = "method_not_found", value = methodName, type = typeFullName });
        }

        if (!string.IsNullOrWhiteSpace(signatureFilter))
        {
            var requested = SplitSignature(signatureFilter);
            candidates = candidates.Where(m => MatchSignature(m, requested)).ToList();

            if (candidates.Count == 0)
            {
                throw new RpcException(
                    -32602,
                    "Invalid params",
                    new
                    {
                        field = "signature",
                        reason = "no_overload_matches",
                        value = signatureFilter,
                    });
            }
        }

        if (candidates.Count > 1)
        {
            var sigs = candidates.Select(BuildSignatureLabel).ToArray();
            throw new RpcException(
                -32602,
                "Invalid params",
                new
                {
                    field = "signature",
                    reason = "ambiguous_overload",
                    hint = "Pass signature='Type1,Type2' (FullName or short name) to disambiguate.",
                    available = sigs,
                });
        }

        var picked = candidates[0];
        string csharp;
        try
        {
            csharp = decompiler.DecompileAsString(picked.MetadataToken);
        }
        catch (Exception ex)
        {
            throw new RpcException(
                -32603,
                "Internal error",
                new { reason = "decompile_failed", message = ex.Message, type = typeFullName, method = methodName });
        }

        var summary = LookupSummary(reflectionType, picked, isCtor);

        return new DecompileResult
        {
            Type = typeFullName,
            Method = isCtor ? ".ctor" : picked.Name,
            Signature = BuildSignatureLabel(picked),
            CSharp = csharp,
            Summary = summary,
        };
    }

    private static string LookupSummary(Type reflectionType, IMethod picked, bool isCtor)
    {
        var reflectionMethod = FindReflectionMethod(reflectionType, picked, isCtor);
        if (reflectionMethod is null)
        {
            return "";
        }

        var docs = XmlDocLoader.ForAssembly(reflectionType.Assembly);
        if (docs is null)
        {
            return "";
        }

        var id = DocCommentId.ForMethod(reflectionMethod);
        return docs.Lookup(id)?.Summary ?? "";
    }

    private static MethodBase? FindReflectionMethod(Type reflectionType, IMethod ilSpyMethod, bool isCtor)
    {
        const BindingFlags flags = BindingFlags.Public | BindingFlags.NonPublic
            | BindingFlags.Instance | BindingFlags.Static | BindingFlags.DeclaredOnly;

        var expectedParamTypeNames = ilSpyMethod.Parameters
            .Select(p => p.Type.FullName ?? p.Type.Name)
            .ToArray();

        IEnumerable<MethodBase> candidates = isCtor
            ? reflectionType.GetConstructors(flags).Cast<MethodBase>()
            : reflectionType.GetMethods(flags).Where(m => m.Name == ilSpyMethod.Name).Cast<MethodBase>();

        foreach (var m in candidates)
        {
            var rParams = m.GetParameters();
            if (rParams.Length != expectedParamTypeNames.Length)
            {
                continue;
            }

            var match = true;
            for (var i = 0; i < rParams.Length; i++)
            {
                var actualType = rParams[i].ParameterType;
                if (actualType.IsByRef)
                {
                    actualType = actualType.GetElementType() ?? actualType;
                }
                var actualName = actualType.FullName ?? actualType.Name;
                if (!string.Equals(actualName, expectedParamTypeNames[i], StringComparison.Ordinal))
                {
                    match = false;
                    break;
                }
            }

            if (match)
            {
                return m;
            }
        }

        return null;
    }

    private static string? TryGetAssemblyPath(Assembly assembly)
    {
        try
        {
            var location = assembly.Location;
            return string.IsNullOrEmpty(location) || !File.Exists(location) ? null : location;
        }
        catch
        {
            return null;
        }
    }

    private static CSharpDecompiler CreateDecompiler(string assemblyPath)
    {
        var settings = new DecompilerSettings
        {
            LoadInMemory = true,
            ThrowOnAssemblyResolveErrors = false,
        };

        var stream = new FileStream(assemblyPath, FileMode.Open, FileAccess.Read);
        var pe = new PEFile(
            assemblyPath,
            stream,
            PEStreamOptions.PrefetchEntireImage,
            MetadataReaderOptions.Default);

        var resolver = new AppDomainAssemblyResolver(assemblyPath, pe);
        return new CSharpDecompiler(pe, resolver, settings);
    }

    private static string[] SplitSignature(string sig)
    {
        var trimmed = sig.Trim();
        if (trimmed.StartsWith("(") && trimmed.EndsWith(")"))
        {
            trimmed = trimmed.Substring(1, trimmed.Length - 2);
        }
        if (string.IsNullOrWhiteSpace(trimmed))
        {
            return Array.Empty<string>();
        }
        return trimmed.Split(',').Select(x => x.Trim()).ToArray();
    }

    private static bool MatchSignature(IMethod m, string[] requested)
    {
        var pars = m.Parameters;
        if (pars.Count != requested.Length)
        {
            return false;
        }

        for (var i = 0; i < pars.Count; i++)
        {
            var actualFull = pars[i].Type.FullName ?? pars[i].Type.Name;
            var actualShort = pars[i].Type.Name;
            var req = requested[i];
            if (!string.Equals(actualFull, req, StringComparison.Ordinal)
                && !string.Equals(actualShort, req, StringComparison.Ordinal))
            {
                return false;
            }
        }
        return true;
    }

    private static string BuildSignatureLabel(IMethod m)
    {
        var paramNames = string.Join(",", m.Parameters.Select(p => p.Type.FullName ?? p.Type.Name));
        return "(" + paramNames + ")";
    }
}
