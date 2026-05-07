# C\#

`zerodds-cs` is a P/Invoke binding that calls into `libzerodds` from
.NET / Mono / Unity.

## NuGet (when published)

```xml
<PackageReference Include="ZeroDDS" Version="0.0.0-pre" />
```

Until the NuGet package lands, build the project against the
`crates/cs/` source.

## Native runtime requirement

The .NET binding loads `libzerodds.so` (Linux), `libzerodds.dylib`
(macOS), or `zerodds.dll` (Windows) at runtime. Install the
matching native package first
([01 Getting Started → installation](../01-getting-started/installation.md)).

## Hello, world

```csharp
using System;
using ZeroDDS;

class Program {
    static void Main() {
        using var rt = Runtime.Create(domainId: 0);
        using var w  = rt.CreateWriter("Hello", "RawBytes", reliable: true);

        if (!w.WaitForMatched(1, TimeSpan.FromSeconds(5))) {
            Console.Error.WriteLine("no subscriber"); return;
        }

        var bytes = System.Text.Encoding.UTF8.GetBytes("hello from C#");
        w.Write(bytes);
    }
}
```

Subscriber:

```csharp
using var r = rt.CreateReader("Hello", "RawBytes", reliable: true);
while (true) {
    if (r.Take() is byte[] payload) {
        Console.WriteLine($"got: {System.Text.Encoding.UTF8.GetString(payload)}");
    }
    Thread.Sleep(10);
}
```

## Disposal

Every wrapper implements `IDisposable`. Always wrap in `using`
or call `.Dispose()` explicitly — leaks here mean leaked native
handles.

## Generated types from IDL

`zerodds-idlc Robot.idl --csharp -o gen/cs` produces:

```csharp
namespace Robot {
    [DataContract]
    public class Pose {
        [DataMember(Order = 0)] public string Id { get; set; }
        [DataMember(Order = 1)] public double X { get; set; }
        [DataMember(Order = 2)] public double Y { get; set; }
        [DataMember(Order = 3)] public double Z { get; set; }

        public byte[] EncodeCdr() { /* generated */ }
        public static Pose DecodeCdr(byte[] bytes) { /* generated */ }
    }
}
```

Use with the generic API:

```csharp
using var w = rt.CreateTypedWriter<Robot.Pose>("Telemetry");
w.Write(new Robot.Pose { Id = "r1", X = 1.0, Y = 2.0, Z = 3.0 });
```

## Threading

The native runtime is thread-safe; the C# wrappers don't add
extra synchronisation. From `Task.Run` blocks works fine.

## Marshalling

The binding uses `unsafe` blocks under the hood for
P/Invoke buffer passing — these are encapsulated in the
wrapper so application code stays in safe-managed mode.

## Unity / Mono

Drop `libzerodds.so` (or platform equivalent) into
`Assets/Plugins/<arch>/`. The C# wrappers compile against
.NET Standard 2.0, so they work in Unity (`Mono` and `IL2CPP`
backends).

## Reading further

- `crates/cs/README.md` — pre-release notes.
- `crates/cs/examples/` — sample apps + Unity scene.
- [c.md](c.md) — for the underlying ABI.
