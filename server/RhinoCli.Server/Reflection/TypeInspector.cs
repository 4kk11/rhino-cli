using System.Reflection;

namespace RhinoCli.Server.Reflection;

public enum InspectBinding
{
    Public,
    PublicInstance,
    PublicStatic,
    NonPublic,
    All,
}

public sealed record InspectOptions(
    InspectBinding Binding = InspectBinding.Public,
    bool IncludeInherited = false);

public static class TypeInspector
{
    public static InspectBinding ParseBinding(string? value)
    {
        return (value ?? "").Trim().ToLowerInvariant() switch
        {
            "" or "public" => InspectBinding.Public,
            "public_instance" => InspectBinding.PublicInstance,
            "public_static" => InspectBinding.PublicStatic,
            "non_public" => InspectBinding.NonPublic,
            "all" => InspectBinding.All,
            _ => throw new RpcException(
                -32602,
                "Invalid params",
                new { field = "binding", expected = "one of: public, public_instance, public_static, non_public, all", actual = value }),
        };
    }

    public static Type? ResolveType(string fullName)
    {
        if (string.IsNullOrWhiteSpace(fullName))
        {
            return null;
        }

        var type = Type.GetType(fullName, throwOnError: false);
        if (type is not null)
        {
            return type;
        }

        foreach (var assembly in AppDomain.CurrentDomain.GetAssemblies())
        {
            type = assembly.GetType(fullName, throwOnError: false);
            if (type is not null)
            {
                return type;
            }
        }

        return null;
    }

    public static object Inspect(string fullName, InspectOptions options)
    {
        var type = ResolveType(fullName)
            ?? throw new RpcException(
                -32602,
                "Invalid params",
                new
                {
                    field = "name",
                    reason = "type_not_found",
                    value = fullName,
                    hint = "Use rhino.search_types to discover the fully qualified name."
                });

        return BuildDescription(type, options, XmlDocLoader.ForAssembly(type.Assembly));
    }

    /// <summary>
    /// Inspect a type using an explicit XML doc loader. Used by tests that supply a fixture.
    /// </summary>
    public static object Inspect(Type type, InspectOptions options, XmlDocLoader? xmlDocs)
    {
        return BuildDescription(type, options, xmlDocs);
    }

    private static object BuildDescription(Type type, InspectOptions opts, XmlDocLoader? typeAssemblyDocs)
    {
        var memberFlags = ToFlags(opts.Binding, opts.IncludeInherited);
        // Constructors do not inherit, so DeclaredOnly is always applied for ctors.
        var ctorFlags = ToFlags(opts.Binding, includeInherited: false) & ~BindingFlags.Static;

        var kind = type.IsEnum
            ? "enum"
            : type.IsInterface
                ? "interface"
                : type.IsValueType
                    ? "struct"
                    : "class";

        var typeSummary = typeAssemblyDocs?.Lookup(DocCommentId.ForType(type))?.Summary ?? "";

        var constructors = type.GetConstructors(ctorFlags)
            .Select(c =>
            {
                var docs = DocsForMember(c, typeAssemblyDocs);
                var entry = docs.loader?.Lookup(DocCommentId.ForMethod(c));
                return new
                {
                    @params = c.GetParameters().Select(p => BuildParam(p, entry?.Params)).ToArray(),
                    is_public = c.IsPublic,
                    summary = entry?.Summary ?? "",
                };
            })
            .ToArray();

        var properties = type.GetProperties(memberFlags)
            .Select(p =>
            {
                var docs = DocsForMember(p, typeAssemblyDocs);
                var entry = docs.loader?.Lookup(DocCommentId.ForProperty(p));
                return new
                {
                    name = p.Name,
                    type = FormatType(p.PropertyType),
                    get = p.CanRead,
                    set = p.CanWrite,
                    @static = (p.GetMethod ?? p.SetMethod)?.IsStatic ?? false,
                    summary = entry?.Summary ?? "",
                };
            })
            .ToArray();

        var methods = type.GetMethods(memberFlags)
            .Where(m => !m.IsSpecialName)
            .GroupBy(m => (m.Name, m.IsStatic))
            .Select(g => new
            {
                name = g.Key.Name,
                @static = g.Key.IsStatic,
                overloads = g.Select(m =>
                {
                    var docs = DocsForMember(m, typeAssemblyDocs);
                    var entry = docs.loader?.Lookup(DocCommentId.ForMethod(m));
                    return new
                    {
                        @params = m.GetParameters().Select(p => BuildParam(p, entry?.Params)).ToArray(),
                        return_type = FormatType(m.ReturnType),
                        is_generic = m.IsGenericMethodDefinition,
                        generic_args = m.GetGenericArguments().Select(t => t.Name).ToArray(),
                        summary = entry?.Summary ?? "",
                        returns = entry?.Returns ?? "",
                    };
                }).ToArray(),
            })
            .ToArray();

        var events = type.GetEvents(memberFlags)
            .Select(e =>
            {
                var docs = DocsForMember(e, typeAssemblyDocs);
                var entry = docs.loader?.Lookup(DocCommentId.ForEvent(e));
                return new
                {
                    name = e.Name,
                    handler_type = FormatType(e.EventHandlerType ?? typeof(object)),
                    summary = entry?.Summary ?? "",
                };
            })
            .ToArray();

        var fields = type.GetFields(memberFlags)
            .Select(f =>
            {
                var docs = DocsForMember(f, typeAssemblyDocs);
                var entry = docs.loader?.Lookup(DocCommentId.ForField(f));
                return new
                {
                    name = f.Name,
                    type = FormatType(f.FieldType),
                    @static = f.IsStatic,
                    is_literal = f.IsLiteral,
                    summary = entry?.Summary ?? "",
                };
            })
            .ToArray();

        return new
        {
            full_name = type.FullName ?? type.Name,
            assembly = type.Assembly.GetName().Name ?? "",
            kind,
            is_abstract = type.IsAbstract && !type.IsInterface && !type.IsEnum,
            is_sealed = type.IsSealed && !type.IsEnum,
            base_type = type.BaseType?.FullName,
            interfaces = type.GetInterfaces().Select(FormatType).ToArray(),
            summary = typeSummary,
            constructors,
            properties,
            methods,
            events,
            fields,
        };
    }

    private static (XmlDocLoader? loader, Type? declaringType) DocsForMember(MemberInfo member, XmlDocLoader? fallback)
    {
        var declaringType = member.DeclaringType;
        if (declaringType is null)
        {
            return (fallback, null);
        }

        var docs = XmlDocLoader.ForAssembly(declaringType.Assembly) ?? fallback;
        return (docs, declaringType);
    }

    private static object BuildParam(ParameterInfo p, IReadOnlyDictionary<string, string>? paramDocs)
    {
        var paramType = p.ParameterType;
        var isOut = p.IsOut;
        var isRef = paramType.IsByRef && !isOut;
        var summary = (paramDocs is not null
                       && p.Name is not null
                       && paramDocs.TryGetValue(p.Name, out var s))
            ? s
            : "";
        return new
        {
            name = p.Name ?? "",
            type = FormatType(paramType),
            is_out = isOut,
            is_ref = isRef,
            has_default = p.HasDefaultValue,
            default_value = p.HasDefaultValue ? p.DefaultValue?.ToString() : null,
            summary,
        };
    }

    public static string FormatType(Type type)
    {
        if (type.IsByRef)
        {
            return FormatType(type.GetElementType()!);
        }

        if (type.IsArray)
        {
            return FormatType(type.GetElementType()!) + "[]";
        }

        if (type.IsGenericType)
        {
            var definition = type.GetGenericTypeDefinition();
            var name = definition.FullName ?? definition.Name;
            var backtick = name.IndexOf('`');
            if (backtick >= 0)
            {
                name = name.Substring(0, backtick);
            }

            var args = string.Join(", ", type.GetGenericArguments().Select(FormatType));
            return $"{name}<{args}>";
        }

        return type.FullName ?? type.Name;
    }

    private static BindingFlags ToFlags(InspectBinding binding, bool includeInherited)
    {
        var flags = binding switch
        {
            InspectBinding.Public => BindingFlags.Public | BindingFlags.Instance | BindingFlags.Static,
            InspectBinding.PublicInstance => BindingFlags.Public | BindingFlags.Instance,
            InspectBinding.PublicStatic => BindingFlags.Public | BindingFlags.Static,
            InspectBinding.NonPublic => BindingFlags.NonPublic | BindingFlags.Public | BindingFlags.Instance | BindingFlags.Static,
            InspectBinding.All => BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance | BindingFlags.Static,
            _ => BindingFlags.Public | BindingFlags.Instance | BindingFlags.Static,
        };

        if (!includeInherited)
        {
            flags |= BindingFlags.DeclaredOnly;
        }

        return flags;
    }
}
