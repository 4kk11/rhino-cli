namespace RhinoCli.Server;

public sealed class RpcException : Exception
{
    public int Code { get; }
    public new object? Data { get; }

    public RpcException(int code, string message, object? data = null)
        : base(message)
    {
        Code = code;
        Data = data;
    }
}
