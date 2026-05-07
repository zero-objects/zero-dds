//! TS-3 — Codegen-Compile-Tests fuer C#.
//!
//! Generiert C#-Source aus IDL und ruft `dotnet build` (mit
//! Inline-csproj) auf. Faengt Code-Drift im idl-csharp-Codegen.
//!
//! **Voraussetzung:** `dotnet` CLI im `PATH` (mind. .NET 6.0+).
//! Tests werden geskippt wenn nicht verfuegbar.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_csharp::{CsGenOptions, generate_csharp};

fn dotnet_available() -> bool {
    Command::new("dotnet")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn check_compiles(src: &str) -> Result<(), String> {
    if !dotnet_available() {
        eprintln!("WARNING: skipping C# compile-check, no dotnet in PATH");
        return Ok(());
    }

    let ast =
        zerodds_idl::parse(src, &ParserConfig::default()).map_err(|e| format!("parse: {e:?}"))?;
    let cs_source =
        generate_csharp(&ast, &CsGenOptions::default()).map_err(|e| format!("gen: {e:?}"))?;

    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    // Inline-csproj: minimaler library-target, .NET 8.0.
    let csproj = r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net8.0</TargetFramework>
    <Nullable>enable</Nullable>
    <NoWarn>CS0168;CS8019;CS8632</NoWarn>
  </PropertyGroup>
</Project>
"#;
    // Stub-Runtime fuer `Omg.Types.ITopicType<T>` — der Codegen
    // emittiert `using Omg.Types;` und `: ITopicType<T>`-Implementierungen.
    // Im echten Codepfad ist das die DDS-CSharp-PSM-Runtime.
    let stub = "namespace Omg.Types { \
                using System.Collections; using System.Collections.Generic; \
                public interface ITopicType<T> {} \
                public interface ISequence<T> : System.Collections.Generic.IList<T> {} \
                public interface IBoundedSequence<T> : ISequence<T> { int Bound { get; } } \
                public sealed class SequenceList<T> : ISequence<T> { \
                    private readonly List<T> _items = new(); \
                    public int Count => _items.Count; \
                    public bool IsReadOnly => false; \
                    public T this[int i] { get => _items[i]; set => _items[i] = value; } \
                    public void Add(T x) => _items.Add(x); \
                    public void Insert(int i, T x) => _items.Insert(i, x); \
                    public void Clear() => _items.Clear(); \
                    public bool Contains(T x) => _items.Contains(x); \
                    public void CopyTo(T[] a, int i) => _items.CopyTo(a, i); \
                    public IEnumerator<T> GetEnumerator() => _items.GetEnumerator(); \
                    public int IndexOf(T x) => _items.IndexOf(x); \
                    public bool Remove(T x) => _items.Remove(x); \
                    public void RemoveAt(int i) => _items.RemoveAt(i); \
                    IEnumerator IEnumerable.GetEnumerator() => _items.GetEnumerator(); \
                } \
                }\n";
    // Stub-Runtime fuer `ZeroDDS.Cdr` — der Codegen emittiert
    // `*TypeSupport`-Klassen die `IDdsTopicType<T>` aus dieser
    // Library implementieren. Im echten Codepfad ist das
    // `crates/cs/csharp/ZeroDDS.Cdr/` (separates DLL).
    let cdr_stub = "namespace ZeroDDS.Cdr { \
                using System; using System.Collections.Generic; \
                public enum EndianMode { LittleEndian, BigEndian } \
                public enum ExtensibilityKind { Final, Appendable, Mutable } \
                public sealed class XcdrException : System.Exception { public XcdrException(string m) : base(m) {} } \
                public interface IDdsTopicType<T> where T : notnull { \
                    string TypeName { get; } \
                    bool IsKeyed { get; } \
                    ExtensibilityKind Extensibility { get; } \
                    byte[] Encode(T sample); \
                    byte[] Encode(T sample, EndianMode endian); \
                    T Decode(System.ReadOnlySpan<byte> bytes); \
                    byte[] KeyHash(T sample); \
                } \
                public readonly struct DHeaderScope : System.IDisposable { public void Dispose() {} } \
                public readonly struct DHeaderReadScope { public int BodyStart {get;} public int BodyEnd {get;} public int PreviousOrigin {get;} } \
                public sealed class Xcdr2Writer { \
                    public Xcdr2Writer() {} \
                    public Xcdr2Writer(EndianMode e) {} \
                    public void Align(int a) {} \
                    public void WriteByte(byte v) {} \
                    public void WriteBytes(System.ReadOnlySpan<byte> d) {} \
                    public void WriteBool(bool v) {} \
                    public void WriteOctet(byte v) {} \
                    public void WriteInt16(short v) {} \
                    public void WriteUInt16(ushort v) {} \
                    public void WriteInt32(int v) {} \
                    public void WriteUInt32(uint v) {} \
                    public void WriteInt64(long v) {} \
                    public void WriteUInt64(ulong v) {} \
                    public void WriteFloat32(float v) {} \
                    public void WriteFloat64(double v) {} \
                    public void WriteWChar(char v) {} \
                    public void WriteString(string v) {} \
                    public void WriteWString(string v) {} \
                    public void WriteSequenceLength(int c) {} \
                    public DHeaderScope BeginAppendable() => default; \
                    public DHeaderScope BeginMutable() => default; \
                    public DHeaderScope BeginDHeader() => default; \
                    public void WriteEmHeader(uint id, int lc, bool mu) {} \
                    public void WriteEmHeaderNextInt(uint id, int lc, bool mu, uint nx) {} \
                    public byte[] ToArray() => System.Array.Empty<byte>(); \
                } \
                public ref struct Xcdr2Reader { \
                    public Xcdr2Reader(System.ReadOnlySpan<byte> b) {} \
                    public Xcdr2Reader(System.ReadOnlySpan<byte> b, EndianMode e) {} \
                    public bool ReadBool() => default; \
                    public byte ReadByte() => default; \
                    public byte ReadOctet() => default; \
                    public short ReadInt16() => default; \
                    public ushort ReadUInt16() => default; \
                    public int ReadInt32() => default; \
                    public uint ReadUInt32() => default; \
                    public long ReadInt64() => default; \
                    public ulong ReadUInt64() => default; \
                    public float ReadFloat32() => default; \
                    public double ReadFloat64() => default; \
                    public char ReadWChar() => default; \
                    public string ReadString() => string.Empty; \
                    public string ReadWString() => string.Empty; \
                    public int ReadSequenceLength() => default; \
                    public DHeaderReadScope BeginDHeader() => default; \
                    public void EndDHeader(DHeaderReadScope s) {} \
                    public bool DHeaderDone(DHeaderReadScope s) => true; \
                    public (uint MemberId, int Lc, bool MustUnderstand) ReadEmHeader() => default; \
                } \
                public static class Md5 { public static byte[] Hash(System.ReadOnlySpan<byte> d) => new byte[16]; } \
                }\n";
    std::fs::write(tmp.path().join("Generated.csproj"), csproj).map_err(|e| e.to_string())?;
    std::fs::write(tmp.path().join("OmgTypesStub.cs"), stub).map_err(|e| e.to_string())?;
    std::fs::write(tmp.path().join("ZeroDDSCdrStub.cs"), cdr_stub).map_err(|e| e.to_string())?;
    std::fs::write(tmp.path().join("Generated.cs"), &cs_source).map_err(|e| e.to_string())?;

    let output = Command::new("dotnet")
        .args(["build", "--nologo", "--verbosity", "quiet"])
        .current_dir(tmp.path())
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "dotnet build FAILED:\n--- source ---\n{cs_source}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
        ))
    }
}

#[test]
fn compiles_simple_struct() {
    check_compiles("struct Point { long x; long y; };").expect("simple struct must compile");
}

#[test]
fn compiles_struct_with_string_sequence() {
    check_compiles("struct Bag { string name; sequence<long> ids; };")
        .expect("string+seq must compile");
}

#[test]
fn compiles_module_nesting() {
    check_compiles("module Outer { module Inner { struct S { long x; }; }; };")
        .expect("nested modules must compile");
}

#[test]
fn compiles_enum() {
    check_compiles("enum Color { RED, GREEN, BLUE };").expect("enum must compile");
}

#[test]
fn compiles_union() {
    check_compiles(
        "union U switch (long) { case 1: long a; case 2: double b; default: octet c; };",
    )
    .expect("union must compile");
}

#[test]
fn compiles_typedef() {
    check_compiles("typedef long Counter;").expect("typedef must compile");
}

#[test]
fn compiles_inheritance() {
    check_compiles("struct Base { long base_field; }; struct Child : Base { long child_field; };")
        .expect("inheritance must compile");
}
