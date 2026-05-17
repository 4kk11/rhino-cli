using System.Reflection;
using System.Text;

namespace RhinoCli.Server.Reflection;

/// <summary>
/// Builds the XML documentation member ID strings used in `.xml` doc files
/// (T:, M:, P:, F:, E:) per the C# language spec for documentation comments.
/// </summary>
public static class DocCommentId
{
    public static string ForType(Type type)
    {
        return "T:" + DeclaringTypeName(type);
    }

    public static string ForProperty(PropertyInfo property)
    {
        var sb = new StringBuilder();
        sb.Append("P:").Append(DeclaringTypeName(property.DeclaringType!)).Append('.').Append(property.Name);
        var indexParams = property.GetIndexParameters();
        if (indexParams.Length > 0)
        {
            AppendParameterList(sb, indexParams);
        }
        return sb.ToString();
    }

    public static string ForField(FieldInfo field)
    {
        return "F:" + DeclaringTypeName(field.DeclaringType!) + "." + field.Name;
    }

    public static string ForEvent(EventInfo evt)
    {
        return "E:" + DeclaringTypeName(evt.DeclaringType!) + "." + evt.Name;
    }

    public static string ForMethod(MethodBase method)
    {
        var sb = new StringBuilder();
        sb.Append("M:").Append(DeclaringTypeName(method.DeclaringType!)).Append('.');

        if (method is ConstructorInfo)
        {
            sb.Append("#ctor");
        }
        else
        {
            sb.Append(method.Name);
            if (method.IsGenericMethod)
            {
                sb.Append("``").Append(method.GetGenericArguments().Length);
            }
        }

        AppendParameterList(sb, method.GetParameters());

        if (method is MethodInfo mi && (mi.Name == "op_Implicit" || mi.Name == "op_Explicit"))
        {
            sb.Append('~').Append(ParameterTypeName(mi.ReturnType));
        }

        return sb.ToString();
    }

    private static void AppendParameterList(StringBuilder sb, ParameterInfo[] parameters)
    {
        if (parameters.Length == 0)
        {
            return;
        }

        sb.Append('(');
        for (var i = 0; i < parameters.Length; i++)
        {
            if (i > 0)
            {
                sb.Append(',');
            }
            sb.Append(ParameterTypeName(parameters[i].ParameterType));
        }
        sb.Append(')');
    }

    private static string DeclaringTypeName(Type type)
    {
        // Type ID form: keep backtick arity (`N) but replace nested '+' separator with '.'.
        var name = type.FullName ?? type.Name;
        return name.Replace('+', '.');
    }

    private static string ParameterTypeName(Type type)
    {
        if (type.IsByRef)
        {
            return ParameterTypeName(type.GetElementType()!) + "@";
        }

        if (type.IsPointer)
        {
            return ParameterTypeName(type.GetElementType()!) + "*";
        }

        if (type.IsArray)
        {
            var rank = type.GetArrayRank();
            var element = ParameterTypeName(type.GetElementType()!);
            if (rank == 1)
            {
                return element + "[]";
            }
            // Multi-dimensional: [0:,0:,...]
            var dims = string.Join(",", Enumerable.Repeat("0:", rank));
            return element + "[" + dims + "]";
        }

        if (type.IsGenericParameter)
        {
            return (type.DeclaringMethod is not null ? "``" : "`") + type.GenericParameterPosition;
        }

        if (type.IsGenericType)
        {
            var def = type.GetGenericTypeDefinition();
            var name = (def.FullName ?? def.Name).Replace('+', '.');
            var backtick = name.IndexOf('`');
            if (backtick >= 0)
            {
                name = name.Substring(0, backtick);
            }
            var args = type.GetGenericArguments();
            return name + "{" + string.Join(",", args.Select(ParameterTypeName)) + "}";
        }

        return (type.FullName ?? type.Name).Replace('+', '.');
    }
}
