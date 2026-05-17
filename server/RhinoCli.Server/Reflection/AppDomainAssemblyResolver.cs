using System.Reflection;
using System.Reflection.Metadata;
using System.Reflection.PortableExecutable;
using ICSharpCode.Decompiler.Metadata;

namespace RhinoCli.Server.Reflection;

internal sealed class AppDomainAssemblyResolver : IAssemblyResolver
{
    private readonly UniversalAssemblyResolver _fallback;
    private readonly Lazy<Dictionary<string, string>> _appDomainPaths;

    public AppDomainAssemblyResolver(string mainAssemblyPath, PEFile mainModule)
    {
        _fallback = new UniversalAssemblyResolver(
            mainAssemblyPath,
            throwOnError: false,
            mainModule.DetectTargetFrameworkId(),
            mainModule.DetectRuntimePack(),
            PEStreamOptions.PrefetchMetadata,
            MetadataReaderOptions.Default);

        _appDomainPaths = new Lazy<Dictionary<string, string>>(BuildMap);

        foreach (var dir in EnumerateAppDomainDirectories())
        {
            _fallback.AddSearchDirectory(dir);
        }
    }

    public MetadataFile? Resolve(IAssemblyReference reference)
    {
        if (_appDomainPaths.Value.TryGetValue(reference.Name, out var path))
        {
            var pe = TryLoad(path);
            if (pe is not null)
            {
                return pe;
            }
        }
        return _fallback.Resolve(reference);
    }

    public MetadataFile? ResolveModule(MetadataFile mainModule, string moduleName)
        => _fallback.ResolveModule(mainModule, moduleName);

    public Task<MetadataFile?> ResolveAsync(IAssemblyReference reference)
        => Task.Run(() => Resolve(reference));

    public Task<MetadataFile?> ResolveModuleAsync(MetadataFile mainModule, string moduleName)
        => Task.Run(() => ResolveModule(mainModule, moduleName));

    private static PEFile? TryLoad(string path)
    {
        try
        {
            var stream = new FileStream(path, FileMode.Open, FileAccess.Read);
            return new PEFile(path, stream, PEStreamOptions.PrefetchMetadata, MetadataReaderOptions.Default);
        }
        catch
        {
            return null;
        }
    }

    private static Dictionary<string, string> BuildMap()
    {
        var dict = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        foreach (var asm in AppDomain.CurrentDomain.GetAssemblies())
        {
            try
            {
                if (asm.IsDynamic)
                {
                    continue;
                }
                var loc = asm.Location;
                if (string.IsNullOrEmpty(loc) || !File.Exists(loc))
                {
                    continue;
                }
                var name = asm.GetName().Name;
                if (string.IsNullOrEmpty(name) || dict.ContainsKey(name))
                {
                    continue;
                }
                dict[name] = loc;
            }
            catch
            {
            }
        }
        return dict;
    }

    private static IEnumerable<string> EnumerateAppDomainDirectories()
    {
        var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var asm in AppDomain.CurrentDomain.GetAssemblies())
        {
            string? dir = null;
            try
            {
                if (asm.IsDynamic)
                {
                    continue;
                }
                var loc = asm.Location;
                if (string.IsNullOrEmpty(loc))
                {
                    continue;
                }
                dir = Path.GetDirectoryName(loc);
            }
            catch
            {
            }
            if (!string.IsNullOrEmpty(dir) && seen.Add(dir))
            {
                yield return dir;
            }
        }
    }
}
