# `zerodds-xcdr2-csharp` v1.0 — C# / .NET TypeSupport-Codegen

ZeroDDS Vendor-Spec. Implementiert in `crates/idl-csharp` (Codegen) und
`crates/cs/src/Cdr/` (Helper-Library `ZeroDDS.Cdr`). Konformanz gegen
[`zerodds-xcdr2-bindings-conformance-1.0`](zerodds-xcdr2-bindings-conformance-1.0.md).

## §1 Motivation

Es existiert **keine OMG-DDS-CSharp-PSM-Spec**. RTI Connext .NET und
Vortex OpenSplice C# haben jeweils proprietaere Patterns. ZeroDDS
synthesisiert hier eine spec-conforme C#-Bindung.

`crates/idl-csharp` emittiert heute Datenklassen — encode/decode bleibt
dem App-Entwickler ueberlassen. Diese Spec schliesst die Luecke.

## §2 TypeSupport-Pattern

C# nutzt Generic-Interface mit Static-Helper:

```csharp
namespace ZeroDDS.Cdr;

public interface IDdsTopicType<T> where T : notnull {
    string TypeName { get; }
    bool IsKeyed { get; }
    ExtensibilityKind Extensibility { get; }

    byte[] Encode(T sample);
    byte[] Encode(T sample, EndianMode endian);
    T Decode(ReadOnlySpan<byte> bytes);
    byte[] KeyHash(T sample);  // 16 Bytes (MD5)
}

public enum ExtensibilityKind { Final, Appendable, Mutable }
public enum EndianMode { LittleEndian, BigEndian }
```

Generierter Code: pro IDL-`struct` eine `*TypeSupport`-Klasse die
`IDdsTopicType<T>` implementiert (Singleton via `Instance`).

## §3 Required API-Surface

```csharp
public sealed class MyTypeTypeSupport : IDdsTopicType<MyType> {
    public static readonly MyTypeTypeSupport Instance = new();

    public string TypeName => "MyType";
    public bool IsKeyed => false;
    public ExtensibilityKind Extensibility => ExtensibilityKind.Final;

    public byte[] Encode(MyType sample) => Encode(sample, EndianMode.LittleEndian);
    public byte[] Encode(MyType sample, EndianMode endian) {
        var w = new Xcdr2Writer(endian);
        w.WriteInt32(sample.X);
        w.WriteInt32(sample.Y);
        return w.ToArray();
    }
    public MyType Decode(ReadOnlySpan<byte> bytes) {
        var r = new Xcdr2Reader(bytes, EndianMode.LittleEndian);
        return new MyType { X = r.ReadInt32(), Y = r.ReadInt32() };
    }
    public byte[] KeyHash(MyType sample) => new byte[16]; // !is_keyed
}
```

## §4 Codegen-Pflicht (idl-csharp)

Pro IDL-`struct` MUSS `idl-csharp` emittieren:

1. Datenklasse `MyType` (existiert).
2. **NEU:** `MyTypeTypeSupport : IDdsTopicType<MyType>` mit
   `Encode/Decode/KeyHash/TypeName/IsKeyed/Extensibility`.
3. **NEU:** Static-Helper `Topic<MyType>.Register(participant)` der
   beim Topic-Constructor `MyTypeTypeSupport.Instance` durchreicht.

Generierter Code MUSS in einem geschachtelten `namespace` liegen, der
dem IDL-Modul-Pfad entspricht (z.B. `module Outer { struct S }` →
`namespace Outer; public sealed class S { ... }; public sealed class
STypeSupport : IDdsTopicType<S>`).

## §5 Wire-Type-Mapping

| IDL | C# | Wire (XCDR2 LE) |
|-----|-----|-----------------|
| `boolean` | `bool` | 1 Byte |
| `octet` | `byte` | 1 Byte |
| `char` | `byte` (ASCII) | 1 Byte |
| `wchar` | `char` | 2 Byte LE |
| `short` | `short` | 2 Byte LE Align(2) |
| `unsigned short` | `ushort` | 2 Byte LE Align(2) |
| `long` | `int` | 4 Byte LE Align(4) |
| `unsigned long` | `uint` | 4 Byte LE Align(4) |
| `long long` | `long` | 8 Byte LE Align(8) |
| `unsigned long long` | `ulong` | 8 Byte LE Align(8) |
| `float` | `float` | 4 Byte IEEE-754 LE |
| `double` | `double` | 8 Byte IEEE-754 LE |
| `string` | `string` | uint32 length+1 + UTF-8 + NUL |
| `wstring` | `string` (UTF-16) | uint32 length + UTF-16-LE Code-Units |
| `sequence<T>` | `IList<T>` / `T[]` | uint32 count + T[] |
| `T[N]` | `T[]` | T[] N Elemente |
| nested `struct U` | `U` | rekursiv `UTypeSupport.Instance.Encode(...)` (inline) |
| `enum E` | `enum E : int` | int32 LE |
| `@optional T` | `T?` (Nullable) | M-Flag (Mutable) / 1-Byte present |

## §6 Extensibility

```csharp
// @final
public ExtensibilityKind Extensibility => ExtensibilityKind.Final;
// kein DHEADER

// @appendable (default)
public ExtensibilityKind Extensibility => ExtensibilityKind.Appendable;
// 4-Byte uint32 DHEADER prefixed

// @mutable
public ExtensibilityKind Extensibility => ExtensibilityKind.Mutable;
// PL_CDR2 mit EMHEADER pro Member
```

`Xcdr2Writer.BeginAppendable()` / `BeginMutable()` / `WriteEmHeader(id, lc)`
sind Helper-Methoden. Code lebt in `ZeroDDS.Cdr`.

## §7 Key-Extraction

```csharp
public byte[] KeyHash(Sensor sample) {
    var w = new Xcdr2Writer(EndianMode.BigEndian);
    w.WriteInt32(sample.Id); // @key-Member
    return Md5.Hash(w.ToArray());
}
```

`ZeroDDS.Cdr.Md5` ist eine RFC-1321-Implementation (public-domain
Vendor-Code; `System.Security.Cryptography.MD5` ist OK aber benoetigt
`using`-Block fuer FIPS-Konformitaet).

## §8 Helper-Library `ZeroDDS.Cdr`

`crates/cs/src/Cdr/` (oder als separates `ZeroDDS.Cdr.csproj`):

| Klasse | Zweck |
|--------|-------|
| `IDdsTopicType<T>` | Interface |
| `Xcdr2Writer` | Padding + DHEADER + EMHEADER + Primitive |
| `Xcdr2Reader` | Decoder |
| `ExtensibilityKind`, `EndianMode` | enums |
| `Md5` | RFC 1321 |

Pure C# 11 (.NET 6+). Keine native Dependencies.

## §9 Conformance

L1-L4 gegen [`zerodds-xcdr2-bindings-conformance-1.0`](zerodds-xcdr2-bindings-conformance-1.0.md):

- L1 (Wire): `crates/cs/Tests/Xcdr2WireVectorsTests.cs` (mstest oder
  xunit) prueft V-1..V-12.
- L2 (Codegen): `crates/idl-csharp/tests/snapshots/` mit
  generierten `*TypeSupport`-Klassen.
- L3 (Cross-Lang): `crates/conformance/tests/cross_language_xcdr2.rs`
  ruft `dotnet run` mit pre-built Runner.
- L4 (Cross-Vendor): C# encoded → Cyclone-Subscriber decodes (via
  FFI ueber zerodds-c-api).

## §10 Examples

`crates/cs/Examples/TopicTypedSmoke.cs` ist Referenz-Smoke (generierter
`PointTypeSupport` + Pub/Sub-Loop).

Lauffähige Deep-Examples (sync + async, Sensor-Telemetrie
`Reading { Id, Value, Label }`, über den pure-C#-Wire-Core byte-identisch):

- Sync: [`endpoints/csharp/ExampleSync.cs`](../../endpoints/csharp/ExampleSync.cs)
  — `Client.Poll`-Loop über eine In-Memory-FIFO, voller Feld-Decode.
- Async: [`endpoints/csharp/ExampleAsync.cs`](../../endpoints/csharp/ExampleAsync.cs)
  — `AsyncReader.Stream()` (`IAsyncEnumerable`), Consumer `await foreach`.
- Byte-Identität: [`endpoints/csharp/ByteIdentity.cs`](../../endpoints/csharp/ByteIdentity.cs)
  (@final Golden LE+BE).
- Quickstart: [`endpoints/csharp/QUICKSTART.md`](../../endpoints/csharp/QUICKSTART.md).

## §11 Errata + Open-Questions

- **§11.1 `Span<byte>` vs `byte[]`**: Decode nimmt `ReadOnlySpan<byte>`
  fuer Zero-Copy. Encode liefert `byte[]` (allocate-on-return). Optional
  `EncodeTo(T sample, IBufferWriter<byte> output)` fuer Streaming.
- **§11.2 Nullable Reference Types**: `IDdsTopicType<T>` hat
  `where T : notnull`. `string?`-Members fuer `@optional string`
  werden als optional marked (M-Flag).
- **§11.3 `init`-Properties**: Generierte Klassen nutzen
  `{ get; init; }`-Pattern (immutable nach Constructor); Decode nutzt
  Object-Initializer-Syntax.
- **§11.4 Source-Generators**: idl-csharp emittiert `.cs`-Files;
  ein Roslyn-Source-Generator ist optional und nicht Bestandteil dieser
  v1.0-Spec.
