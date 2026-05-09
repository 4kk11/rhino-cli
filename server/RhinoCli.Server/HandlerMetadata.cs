namespace RhinoCli.Server;

[AttributeUsage(AttributeTargets.Class, Inherited = true, AllowMultiple = false)]
public sealed class HandlerMetadataAttribute : Attribute
{
    public string Description { get; }
    public string ParamsSchema { get; set; } = "null";
    public string ResultSchema { get; set; } = "object";
    public string[] Examples { get; set; } = Array.Empty<string>();
    public string DedicatedCommand { get; set; } = "";
    public string SideEffects { get; set; } = "";
    public string Category { get; set; } = "plugin";

    public HandlerMetadataAttribute(string description)
    {
        Description = description;
    }

    public HandlerMetadata ToMetadata() => new()
    {
        Description = Description,
        ParamsSchema = ParamsSchema,
        ResultSchema = ResultSchema,
        Examples = Examples,
        DedicatedCommand = DedicatedCommand,
        SideEffects = SideEffects,
        Category = Category
    };
}

public sealed class HandlerMetadata
{
    public string Description { get; init; } = "";
    public string ParamsSchema { get; init; } = "null";
    public string ResultSchema { get; init; } = "object";
    public string[] Examples { get; init; } = Array.Empty<string>();
    public string DedicatedCommand { get; init; } = "";
    public string SideEffects { get; init; } = "";
    public string Category { get; init; } = "plugin";
}

public sealed class HandlerDescriptor
{
    public string Method { get; }
    public string Description { get; }
    public string ParamsSchema { get; }
    public string ResultSchema { get; }
    public string[] Examples { get; }
    public string DedicatedCommand { get; }
    public string SideEffects { get; }
    public string Category { get; }

    public HandlerDescriptor(string method, HandlerMetadata metadata)
    {
        Method = method;
        Description = metadata.Description;
        ParamsSchema = metadata.ParamsSchema;
        ResultSchema = metadata.ResultSchema;
        Examples = metadata.Examples;
        DedicatedCommand = metadata.DedicatedCommand;
        SideEffects = metadata.SideEffects;
        Category = metadata.Category;
    }
}
