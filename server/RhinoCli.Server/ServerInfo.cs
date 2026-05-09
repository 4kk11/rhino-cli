namespace RhinoCli.Server;

public sealed class ServerInfo
{
    public string PluginId { get; set; }
    public int Port { get; set; }
    public string ServerVersion { get; set; }

    public ServerInfo(string pluginId = "unknown", int port = 0, string serverVersion = "0.1.0")
    {
        PluginId = pluginId;
        Port = port;
        ServerVersion = serverVersion;
    }
}
