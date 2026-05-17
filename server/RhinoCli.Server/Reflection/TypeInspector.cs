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

        return BuildDescription(type, options);
    }

    private static object BuildDescription(Type type, InspectOptions opts)
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

        var constructors = type.GetConstructors(ctorFlags)
            .Select(c => new
            {
                @params = c.GetParameters().Select(BuildParam).ToArray(),
                is_public = c.IsPublic,
            })
            .ToArray();

        var properties = type.GetProperties(memberFlags)
            .Select(p => new
            {
                name = p.Name,
                type = FormatType(p.PropertyType),
                get = p.CanRead,
                set = p.CanWrite,
                @static = (p.GetMethod ?? p.SetMethod)?.IsStatic ?? false,
            })
            .ToArray();

        var methods = type.GetMethods(memberFlags)
            .Where(m => !m.IsSpecialName)
            .GroupBy(m => (m.Name, m.IsStatic))
            .Select(g => new
            {
                name = g.Key.Name,
                @static = g.Key.IsStatic,
                overloads = g.Select(m => new
                {
                    @params = m.GetParameters().Select(BuildParam).ToArray(),
                    return_type = FormatType(m.ReturnType),
                    is_generic = m.IsGenericMethodDefinition,
                    generic_args = m.GetGenericArguments().Select(t => t.Name).ToArray(),
                }).ToArray(),
            })
            .ToArray();

        var events = type.GetEvents(memberFlags)
            .Select(e => new
            {
                name = e.Name,
                handler_type = FormatType(e.EventHandlerType ?? typeof(object)),
            })
            .ToArray();

        var fields = type.GetFields(memberFlags)
            .Select(f => new
            {
                name = f.Name,
                type = FormatType(f.FieldType),
                @static = f.IsStatic,
                is_literal = f.IsLiteral,
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
            constructors,
            properties,
            methods,
            events,
            fields,
        };
    }

    private static object BuildParam(ParameterInfo p)
    {
        var paramType = p.ParameterType;
        var isOut = p.IsOut;
        var isRef = paramType.IsByRef && !isOut;
        return new
        {
            name = p.Name ?? "",
            type = FormatType(paramType),
            is_out = isOut,
            is_ref = isRef,
            has_default = p.HasDefaultValue,
            default_value = p.HasDefaultValue ? p.DefaultValue?.ToString() : null,
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
