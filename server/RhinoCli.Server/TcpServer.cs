using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Text.Json.Nodes;

namespace RhinoCli.Server;

public sealed class TcpServer : IDisposable
{
    private readonly int _port;
    private readonly MessageRouter _router;
    private TcpListener? _listener;
    private CancellationTokenSource? _cts;
    private Task? _acceptTask;

    public event Action<string>? OnError;

    public TcpServer(int port, HandlerRegistry registry, string pluginId)
        : this(port, registry, pluginId, null)
    {
    }

    public TcpServer(
        int port,
        HandlerRegistry registry,
        string pluginId,
        Func<IHandler, JsonNode?, object?>? invokeHandler)
    {
        _port = port;
        registry.Info.PluginId = pluginId;
        registry.Info.Port = port;
        _router = new MessageRouter(registry, pluginId, invokeHandler);
    }

    public int ActualPort { get; private set; }

    public void Start()
    {
        if (_listener is not null)
        {
            return;
        }

        _cts = new CancellationTokenSource();
        _listener = new TcpListener(IPAddress.Loopback, _port);
        _listener.Start();
        ActualPort = ((IPEndPoint)_listener.LocalEndpoint).Port;
        _acceptTask = Task.Run(() => AcceptLoopAsync(_cts.Token));
    }

    public void Stop()
    {
        _cts?.Cancel();
        _listener?.Stop();
        _listener = null;
        _cts?.Dispose();
        _cts = null;
    }

    public void Dispose() => Stop();

    private async Task AcceptLoopAsync(CancellationToken ct)
    {
        while (!ct.IsCancellationRequested)
        {
            try
            {
                var client = await _listener!.AcceptTcpClientAsync(ct).ConfigureAwait(false);
                _ = Task.Run(() => HandleClientAsync(client, ct), ct);
            }
            catch (OperationCanceledException)
            {
                break;
            }
            catch (Exception ex)
            {
                ReportError($"accept error: {ex.Message}");
            }
        }
    }

    private async Task HandleClientAsync(TcpClient client, CancellationToken ct)
    {
        try
        {
            await using var stream = client.GetStream();
            using var reader = new StreamReader(stream, Encoding.UTF8);
            await using var writer = new StreamWriter(stream, new UTF8Encoding(false))
            {
                AutoFlush = true,
                NewLine = "\n"
            };

            while (!ct.IsCancellationRequested)
            {
                var line = await reader.ReadLineAsync(ct).ConfigureAwait(false);
                if (line is null)
                {
                    break;
                }

                var response = _router.HandleMessage(line);
                await writer.WriteLineAsync(response.AsMemory(), ct).ConfigureAwait(false);
            }
        }
        catch (OperationCanceledException)
        {
        }
        catch (IOException)
        {
        }
        catch (Exception ex)
        {
            ReportError($"client error: {ex.Message}");
        }
        finally
        {
            client.Close();
        }
    }

    private void ReportError(string message) => OnError?.Invoke(message);
}
