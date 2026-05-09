using RhinoCli.Server.Handlers;

namespace RhinoCli.Server;

public sealed class HandlerRegistry
{
    private readonly Dictionary<string, IHandler> _handlers = new(StringComparer.Ordinal);

    public ServerInfo Info { get; }

    public HandlerRegistry(string pluginId = "unknown", int port = 0, string serverVersion = "0.1.0")
    {
        Info = new ServerInfo(pluginId, port, serverVersion);
        RegisterBuiltIns();
    }

    public void Register(string method, IHandler handler)
    {
        if (string.IsNullOrWhiteSpace(method))
        {
            throw new ArgumentException("Method name must not be empty.", nameof(method));
        }

        _handlers[method] = handler ?? throw new ArgumentNullException(nameof(handler));
    }

    public bool Contains(string method) => _handlers.ContainsKey(method);

    public IReadOnlyList<string> Methods => _handlers.Keys.OrderBy(method => method, StringComparer.Ordinal).ToArray();

    internal bool TryGetHandler(string method, out IHandler handler) => _handlers.TryGetValue(method, out handler!);

    private void RegisterBuiltIns()
    {
        Register("system.ping", new PingHandler(Info));
        Register("system.version", new VersionHandler(Info));
        Register("rpc.list_methods", new ListMethodsHandler(this));
        Register("rpc.list_plugins", new ListPluginsHandler(Info));
    }
}
