using System.Collections.Concurrent;
using System.Reflection;
using System.Xml.Linq;

namespace RhinoCli.Server.Reflection;

/// <summary>
/// Parsed `<summary>` / `<param>` / `<returns>` / `<remarks>` text for one XML doc member.
/// All strings are whitespace-normalized (newlines collapsed to single spaces).
/// </summary>
public sealed class MemberDocs
{
    public string Summary { get; init; } = "";
    public string Returns { get; init; } = "";
    public string Remarks { get; init; } = "";
    public IReadOnlyDictionary<string, string> Params { get; init; }
        = new Dictionary<string, string>();
}

/// <summary>
/// Loads a .NET XML documentation file produced beside an assembly
/// (e.g. RhinoCommon.xml next to RhinoCommon.dll) and indexes its members by ID.
/// Per-assembly results are cached so subsequent inspections are free.
/// </summary>
public sealed class XmlDocLoader
{
    private static readonly ConcurrentDictionary<string, XmlDocLoader?> Cache = new();

    private readonly Dictionary<string, MemberDocs> _byId = new(StringComparer.Ordinal);

    public int Count => _byId.Count;

    public MemberDocs? Lookup(string id) => _byId.TryGetValue(id, out var docs) ? docs : null;

    public static XmlDocLoader? ForAssembly(Assembly assembly)
    {
        if (assembly is null)
        {
            return null;
        }

        var assemblyPath = SafeLocation(assembly);
        if (string.IsNullOrEmpty(assemblyPath))
        {
            return null;
        }

        return Cache.GetOrAdd(assemblyPath, TryLoadForAssemblyPath);
    }

    public static XmlDocLoader LoadFromPath(string xmlPath)
    {
        var loader = new XmlDocLoader();
        loader.LoadInto(xmlPath);
        return loader;
    }

    public static XmlDocLoader LoadFromString(string xmlContent)
    {
        var loader = new XmlDocLoader();
        loader.IngestDocument(XDocument.Parse(xmlContent));
        return loader;
    }

    private static XmlDocLoader? TryLoadForAssemblyPath(string assemblyPath)
    {
        var xmlPath = Path.ChangeExtension(assemblyPath, ".xml");
        if (!File.Exists(xmlPath))
        {
            return null;
        }

        try
        {
            return LoadFromPath(xmlPath);
        }
        catch
        {
            // Corrupt / unreadable XML is treated as "no docs" rather than blowing up reflection.
            return null;
        }
    }

    private void LoadInto(string xmlPath)
    {
        IngestDocument(XDocument.Load(xmlPath));
    }

    private void IngestDocument(XDocument doc)
    {
        var members = doc.Descendants("member");
        foreach (var member in members)
        {
            var id = member.Attribute("name")?.Value;
            if (string.IsNullOrEmpty(id))
            {
                continue;
            }

            var paramDict = new Dictionary<string, string>(StringComparer.Ordinal);
            foreach (var param in member.Elements("param"))
            {
                var paramName = param.Attribute("name")?.Value;
                if (string.IsNullOrEmpty(paramName))
                {
                    continue;
                }
                paramDict[paramName] = NormalizeWhitespace(param.Value);
            }

            _byId[id] = new MemberDocs
            {
                Summary = NormalizeWhitespace(member.Element("summary")?.Value),
                Returns = NormalizeWhitespace(member.Element("returns")?.Value),
                Remarks = NormalizeWhitespace(member.Element("remarks")?.Value),
                Params = paramDict,
            };
        }
    }

    private static string NormalizeWhitespace(string? raw)
    {
        if (string.IsNullOrWhiteSpace(raw))
        {
            return "";
        }

        var parts = raw
            .Replace("\r\n", "\n")
            .Split('\n')
            .Select(line => line.Trim())
            .Where(line => line.Length > 0);

        return string.Join(" ", parts);
    }

    private static string SafeLocation(Assembly assembly)
    {
        try
        {
            return assembly.Location ?? "";
        }
        catch
        {
            return "";
        }
    }
}
