namespace RhinoCliPlugin;

internal static class RhinoCliHistoryBuffer
{
    private static readonly object LockObject = new();
    private static readonly List<string> Lines = new();

    public static void Append(string line)
    {
        lock (LockObject)
        {
            Lines.Add(line);
        }
    }

    public static string Text()
    {
        lock (LockObject)
        {
            return string.Join("\n", Lines);
        }
    }

    public static void Clear()
    {
        lock (LockObject)
        {
            Lines.Clear();
        }
    }
}
