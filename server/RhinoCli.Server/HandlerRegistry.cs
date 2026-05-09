using RhinoCli.Server.Handlers;

namespace RhinoCli.Server;

public sealed class HandlerRegistry
{
    private readonly Dictionary<string, IHandler> _handlers = new(StringComparer.Ordinal);
    private readonly Dictionary<string, HandlerMetadata> _metadata = new(StringComparer.Ordinal);

    public ServerInfo Info { get; }

    public HandlerRegistry(string pluginId = "unknown", int port = 0, string serverVersion = "0.1.0")
    {
        Info = new ServerInfo(pluginId, port, serverVersion);
        RegisterBuiltIns();
    }

    public void Register(string method, IHandler handler)
    {
        Register(method, handler, null);
    }

    public void Register(string method, IHandler handler, HandlerMetadata? metadata)
    {
        if (string.IsNullOrWhiteSpace(method))
        {
            throw new ArgumentException("Method name must not be empty.", nameof(method));
        }

        _handlers[method] = handler ?? throw new ArgumentNullException(nameof(handler));
        _metadata[method] = metadata ?? MetadataFromAttribute(handler) ?? new HandlerMetadata();
    }

    public bool Contains(string method) => _handlers.ContainsKey(method);

    public IReadOnlyList<string> Methods => _handlers.Keys.OrderBy(method => method, StringComparer.Ordinal).ToArray();

    public IReadOnlyList<HandlerDescriptor> Capabilities => Methods
        .Select(method => new HandlerDescriptor(method, _metadata[method]))
        .ToArray();

    internal bool TryGetHandler(string method, out IHandler handler) => _handlers.TryGetValue(method, out handler!);

    internal bool TryGetCapability(string method, out HandlerDescriptor capability)
    {
        if (_handlers.ContainsKey(method))
        {
            capability = new HandlerDescriptor(method, _metadata[method]);
            return true;
        }

        capability = null!;
        return false;
    }

    private void RegisterBuiltIns()
    {
        Register("system.ping", new PingHandler(Info));
        Register("system.version", new VersionHandler(Info));
        Register("rpc.capabilities", new CapabilitiesHandler(this));
        Register("rpc.list_methods", new ListMethodsHandler(this));
        Register("rpc.list_plugins", new ListPluginsHandler(Info));
    }

    private static HandlerMetadata? MetadataFromAttribute(IHandler handler)
    {
        return handler
            .GetType()
            .GetCustomAttributes(typeof(HandlerMetadataAttribute), inherit: true)
            .OfType<HandlerMetadataAttribute>()
            .FirstOrDefault()
            ?.ToMetadata();
    }
}
