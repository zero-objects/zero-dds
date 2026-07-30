//! CS-cluster (#67) — real XCDR2 encode→decode roundtrips against the actual
//! `ZeroDDS.Cdr` runtime library (not the stub).
//!
//! Each test generates C# from IDL, drops a `Program.Main` that encodes a
//! sample and decodes it back through the GENERATED TypeSupport, and asserts —
//! at runtime — that the recovered value equals the original. This is the
//! runtime gate the compile-only `compile_check.rs` cannot give: it proves the
//! `ref`-threaded reader, the map/union/array/typedef/nested-seq codecs
//! actually recover the data over the wire.
//!
//! **Prerequisite:** `dotnet` on PATH (>= .NET 8). Skipped otherwise.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::uninlined_format_args,
    missing_docs
)]

use std::path::Path;
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

/// Absolute path to the real `ZeroDDS.Cdr.csproj` in the workspace.
fn zerodds_cdr_csproj() -> String {
    // CARGO_MANIFEST_DIR = crates/idl-csharp
    let manifest = env!("CARGO_MANIFEST_DIR");
    let p = Path::new(manifest)
        .join("../cs/csharp/ZeroDDS.Cdr/ZeroDDS.Cdr.csproj")
        .canonicalize()
        .expect("ZeroDDS.Cdr.csproj must exist");
    p.to_string_lossy().into_owned()
}

/// Generates C# for `idl`, writes `Program.cs` (= `main_body`), builds an exe
/// that references the real `ZeroDDS.Cdr`, runs it, and returns stdout. Panics
/// (with the full generated source) on build/run failure or non-zero exit.
fn run_roundtrip(idl: &str, main_body: &str) -> Option<String> {
    if !dotnet_available() {
        eprintln!("WARNING: skipping C# roundtrip, no dotnet in PATH");
        return None;
    }
    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let cs_source = generate_csharp(&ast, &CsGenOptions::default()).expect("gen");

    let tmp = tempfile::tempdir().expect("tempdir");
    let csproj = format!(
        r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net8.0</TargetFramework>
    <Nullable>enable</Nullable>
    <RollForward>LatestMajor</RollForward>
    <NoWarn>CS0168;CS8019;CS8632;CS0219</NoWarn>
  </PropertyGroup>
  <ItemGroup>
    <ProjectReference Include="{}" />
  </ItemGroup>
</Project>
"#,
        zerodds_cdr_csproj()
    );
    let program = format!(
        "using System;\nusing ZeroDDS.Cdr;\nusing Omg.Types;\n\npublic static class Program\n{{\n    public static int Main()\n    {{\n{}\n        return 0;\n    }}\n}}\n",
        main_body
    );

    std::fs::write(tmp.path().join("Generated.csproj"), csproj).unwrap();
    std::fs::write(tmp.path().join("Generated.cs"), &cs_source).unwrap();
    std::fs::write(tmp.path().join("Program.cs"), &program).unwrap();

    // Build under the cross-process lock — every C# test builds the shared
    // ZeroDDS.Cdr project, whose obj/ output races across parallel test
    // binaries (CS2012). Then run the built exe WITHOUT the lock so the 25
    // roundtrips stay concurrent; only the build step is serialized.
    {
        let _guard = zerodds_dotnet_build_lock::dotnet_build_guard();
        let build = Command::new("dotnet")
            .args(["build", "--nologo", "--verbosity", "quiet"])
            .current_dir(tmp.path())
            .output()
            .expect("dotnet build");
        if !build.status.success() {
            panic!(
                "dotnet build FAILED:\n--- idl ---\n{idl}\n--- generated ---\n{cs_source}\n--- program ---\n{program}\n--- stdout ---\n{}\n--- stderr ---\n{}",
                String::from_utf8_lossy(&build.stdout),
                String::from_utf8_lossy(&build.stderr)
            );
        }
    }

    let out = Command::new("dotnet")
        .args(["run", "--no-build", "--nologo", "--verbosity", "quiet"])
        .current_dir(tmp.path())
        .output()
        .expect("dotnet run");

    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    if !out.status.success() {
        panic!(
            "dotnet run FAILED:\n--- idl ---\n{idl}\n--- generated ---\n{cs_source}\n--- program ---\n{program}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
        );
    }
    Some(stdout)
}

/// Asserts the roundtrip program printed `OK` (the program self-checks values
/// and only prints OK when every assertion passed).
fn assert_roundtrip_ok(idl: &str, main_body: &str) {
    match run_roundtrip(idl, main_body) {
        None => {} // dotnet missing — skipped
        Some(stdout) => assert!(
            stdout.contains("OK"),
            "roundtrip did not report OK; stdout was:\n{stdout}"
        ),
    }
}

#[test]
fn roundtrip_nested_struct_and_seq_of_struct() {
    // CS-cluster #1: the reader is a `ref struct`; nested struct + seq<struct>
    // decode must thread it by ref or the cursor desyncs. This asserts the
    // recovered values, so a desync corrupts them and fails.
    let idl = "struct Inner { long x; long y; }; \
               struct Outer { Inner a; sequence<Inner> many; long z; };";
    let body = r#"
        var s = new Outer {
            A = new Inner { X = 1, Y = 2 },
            Many = new Omg.Types.SequenceList<Inner> { new Inner { X = 3, Y = 4 }, new Inner { X = 5, Y = 6 } },
            Z = 99,
        };
        var bytes = OuterTypeSupport.Instance.Encode(s);
        var back = OuterTypeSupport.Instance.Decode(bytes);
        if (back.A.X != 1 || back.A.Y != 2) throw new Exception("nested struct desync");
        if (back.Many.Count != 2) throw new Exception("seq count");
        if (back.Many[0].X != 3 || back.Many[0].Y != 4) throw new Exception("seq[0]");
        if (back.Many[1].X != 5 || back.Many[1].Y != 6) throw new Exception("seq[1]");
        if (back.Z != 99) throw new Exception("trailing scalar desync (cursor not threaded)");
        Console.WriteLine("OK");
"#;
    assert_roundtrip_ok(idl, body);
}

#[test]
fn roundtrip_map_member() {
    // CS-cluster #2: map<K,V> must encode+decode (was gated out entirely).
    let idl = "struct M { map<long, string> kv; long n; };";
    let body = r#"
        var s = new M {
            Kv = new System.Collections.Generic.Dictionary<int, string> { { 1, "a" }, { 2, "bb" } },
            N = 7,
        };
        var bytes = MTypeSupport.Instance.Encode(s);
        var back = MTypeSupport.Instance.Decode(bytes);
        if (back.Kv.Count != 2) throw new Exception("map count");
        if (back.Kv[1] != "a" || back.Kv[2] != "bb") throw new Exception("map values");
        if (back.N != 7) throw new Exception("trailing scalar after map");
        Console.WriteLine("OK");
"#;
    assert_roundtrip_ok(idl, body);
}

#[test]
fn roundtrip_union() {
    // CS-cluster #3: union must emit a real codec and round-trip each branch.
    let idl = "union U switch (long) { case 1: long a; case 2: double b; default: octet c; };";
    let body = r#"
        var u1 = new U { Discriminator = 1, Value = (int)42 };
        var b1 = UTypeSupport.Instance.Encode(u1);
        var r1 = UTypeSupport.Instance.Decode(b1);
        if (r1.Discriminator != 1 || (int)r1.Value! != 42) throw new Exception("union case 1");

        var u2 = new U { Discriminator = 2, Value = (double)3.5 };
        var b2 = UTypeSupport.Instance.Encode(u2);
        var r2 = UTypeSupport.Instance.Decode(b2);
        if (r2.Discriminator != 2 || (double)r2.Value! != 3.5) throw new Exception("union case 2");

        var u3 = new U { Discriminator = 9, Value = (byte)200 };
        var b3 = UTypeSupport.Instance.Encode(u3);
        var r3 = UTypeSupport.Instance.Decode(b3);
        if ((byte)r3.Value! != 200) throw new Exception("union default");
        Console.WriteLine("OK");
"#;
    assert_roundtrip_ok(idl, body);
}

#[test]
fn roundtrip_union_enum_discriminator() {
    // CS-cluster #3 (enum-discriminated union): the case labels must be
    // qualified (`Kind.K_A`) and the discriminator round-trips as int32.
    let idl = "enum Kind { K_A, K_B, K_C }; \
               union EU switch (Kind) { case K_A: long a; case K_B: short b; default: octet c; };";
    let body = r#"
        var u1 = new EU { Discriminator = Kind.K_A, Value = (int)123 };
        var b1 = EUTypeSupport.Instance.Encode(u1);
        var r1 = EUTypeSupport.Instance.Decode(b1);
        if (r1.Discriminator != Kind.K_A || (int)r1.Value! != 123) throw new Exception("enum union K_A");

        var u2 = new EU { Discriminator = Kind.K_B, Value = (short)7 };
        var b2 = EUTypeSupport.Instance.Encode(u2);
        var r2 = EUTypeSupport.Instance.Decode(b2);
        if (r2.Discriminator != Kind.K_B || (short)r2.Value! != 7) throw new Exception("enum union K_B");

        var u3 = new EU { Discriminator = Kind.K_C, Value = (byte)9 };
        var b3 = EUTypeSupport.Instance.Encode(u3);
        var r3 = EUTypeSupport.Instance.Decode(b3);
        if (r3.Discriminator != Kind.K_C || (byte)r3.Value! != 9) throw new Exception("enum union default");
        Console.WriteLine("OK");
"#;
    assert_roundtrip_ok(idl, body);
}

#[test]
fn roundtrip_fixed_array_member() {
    // CS-cluster #4: fixed arrays (1-D and 2-D) must round-trip element-wise
    // with no length prefix.
    let idl = "struct A { long v[3]; long grid[2][2]; long n; };";
    let body = r#"
        var s = new A {
            V = new int[] { 10, 20, 30 },
            Grid = new int[][] { new int[] { 1, 2 }, new int[] { 3, 4 } },
            N = 5,
        };
        var bytes = ATypeSupport.Instance.Encode(s);
        var back = ATypeSupport.Instance.Decode(bytes);
        if (back.V.Length != 3 || back.V[0] != 10 || back.V[1] != 20 || back.V[2] != 30) throw new Exception("1D array");
        if (back.Grid.Length != 2 || back.Grid[0][0] != 1 || back.Grid[0][1] != 2 || back.Grid[1][0] != 3 || back.Grid[1][1] != 4) throw new Exception("2D array");
        if (back.N != 5) throw new Exception("trailing scalar after array");
        Console.WriteLine("OK");
"#;
    assert_roundtrip_ok(idl, body);
}

#[test]
fn roundtrip_typedef_to_primitive() {
    // CS-cluster #5: typedef-to-primitive wrapper record must be unwrapped on
    // encode and re-wrapped on decode.
    let idl = "typedef double CurrentInAmpsType; \
               struct T { CurrentInAmpsType battery; long n; };";
    let body = r#"
        var s = new T { Battery = new CurrentInAmpsType(12.5), N = 3 };
        var bytes = TTypeSupport.Instance.Encode(s);
        var back = TTypeSupport.Instance.Decode(bytes);
        if (back.Battery.Value != 12.5) throw new Exception("typedef value");
        if (back.N != 3) throw new Exception("trailing scalar after typedef");
        Console.WriteLine("OK");
"#;
    assert_roundtrip_ok(idl, body);
}

#[test]
fn roundtrip_nested_sequence() {
    // CS-cluster #6: sequence<sequence<long>> must use depth-unique temps and
    // round-trip both levels.
    let idl = "struct N { sequence<sequence<long>> mat; long tail; };";
    let body = r#"
        var inner0 = new Omg.Types.SequenceList<int> { 1, 2 };
        var inner1 = new Omg.Types.SequenceList<int> { 3, 4, 5 };
        var s = new N {
            Mat = new Omg.Types.SequenceList<Omg.Types.ISequence<int>> { inner0, inner1 },
            Tail = 77,
        };
        var bytes = NTypeSupport.Instance.Encode(s);
        var back = NTypeSupport.Instance.Decode(bytes);
        if (back.Mat.Count != 2) throw new Exception("outer count");
        if (back.Mat[0].Count != 2 || back.Mat[0][0] != 1 || back.Mat[0][1] != 2) throw new Exception("inner0");
        if (back.Mat[1].Count != 3 || back.Mat[1][0] != 3 || back.Mat[1][2] != 5) throw new Exception("inner1");
        if (back.Tail != 77) throw new Exception("trailing scalar after nested seq");
        Console.WriteLine("OK");
"#;
    assert_roundtrip_ok(idl, body);
}

#[test]
fn roundtrip_optional_absent_stays_null() {
    // CS2 (1): an ABSENT @optional member must decode to `null`, not the
    // value-type default. Before the fix the decode local was typed `double`
    // (non-nullable) and `default!`-initialised, so an absent `@optional double`
    // came back as 0.0 — indistinguishable from a present zero. A present
    // optional must still recover its value (and a present zero must NOT be
    // mistaken for absence).
    let idl = "@appendable struct O { long req; @optional double maybe; @optional long note; long tail; };";
    let body = r#"
        // (a) absent optionals -> null
        var sAbsent = new O { Req = 5, Maybe = null, Note = null, Tail = 9 };
        var bAbsent = OTypeSupport.Instance.Encode(sAbsent);
        var rAbsent = OTypeSupport.Instance.Decode(bAbsent);
        if (rAbsent.Req != 5) throw new Exception("req");
        if (rAbsent.Maybe != null) throw new Exception("absent optional double must be null, was " + rAbsent.Maybe);
        if (rAbsent.Note != null) throw new Exception("absent optional long must be null");
        if (rAbsent.Tail != 9) throw new Exception("trailing scalar after absent optionals");

        // (b) present optionals -> value (incl. a present ZERO, which must NOT
        // be confused with absence)
        var sPresent = new O { Req = 5, Maybe = 0.0, Note = 42, Tail = 9 };
        var bPresent = OTypeSupport.Instance.Encode(sPresent);
        var rPresent = OTypeSupport.Instance.Decode(bPresent);
        if (rPresent.Maybe is null) throw new Exception("present optional zero must NOT be null");
        if (rPresent.Maybe != 0.0) throw new Exception("present optional zero value");
        if (rPresent.Note != 42) throw new Exception("present optional long value");
        if (rPresent.Tail != 9) throw new Exception("trailing scalar after present optionals");
        Console.WriteLine("OK");
"#;
    assert_roundtrip_ok(idl, body);
}

#[test]
fn roundtrip_optional_absent_stays_null_mutable() {
    // CS2 (1), @mutable path: the same null-vs-zero distinction must hold when
    // the decode loop assigns into id-keyed locals.
    let idl =
        "@mutable struct OM { @id(1) long req; @id(2) @optional double maybe; @id(3) long tail; };";
    let body = r#"
        var sAbsent = new OM { Req = 1, Maybe = null, Tail = 2 };
        var rAbsent = OMTypeSupport.Instance.Decode(OMTypeSupport.Instance.Encode(sAbsent));
        if (rAbsent.Maybe != null) throw new Exception("mutable absent optional must be null");
        if (rAbsent.Req != 1 || rAbsent.Tail != 2) throw new Exception("mutable scalars");

        var sPresent = new OM { Req = 1, Maybe = 0.0, Tail = 2 };
        var rPresent = OMTypeSupport.Instance.Decode(OMTypeSupport.Instance.Encode(sPresent));
        if (rPresent.Maybe is null || rPresent.Maybe != 0.0) throw new Exception("mutable present optional zero");
        Console.WriteLine("OK");
"#;
    assert_roundtrip_ok(idl, body);
}

#[test]
fn roundtrip_typedef_to_sequence_member() {
    // CS2 (2): a typedef-to-SEQUENCE used as a member must be codecable — the
    // alias wrapper record `Vec(ISequence<int> Value)` is unwrapped on encode
    // and re-wrapped on decode (the inner sequence decode is a loop, so it can't
    // use the single-expression unwrap path). Before the fix the containing
    // struct got NO TypeSupport at all (gated out).
    let idl = "typedef sequence<long> Vec; \
               struct TS { Vec data; long tail; };";
    let body = r#"
        var s = new TS {
            Data = new Vec(new Omg.Types.SequenceList<int> { 10, 20, 30 }),
            Tail = 88,
        };
        var bytes = TSTypeSupport.Instance.Encode(s);
        var back = TSTypeSupport.Instance.Decode(bytes);
        if (back.Data.Value.Count != 3) throw new Exception("typedef-seq count");
        if (back.Data.Value[0] != 10 || back.Data.Value[1] != 20 || back.Data.Value[2] != 30) throw new Exception("typedef-seq values");
        if (back.Tail != 88) throw new Exception("trailing scalar after typedef-seq");
        Console.WriteLine("OK");
"#;
    assert_roundtrip_ok(idl, body);
}

#[test]
fn roundtrip_typedef_to_map_member() {
    // CS2 (2): a typedef-to-MAP used as a member must be codecable too.
    let idl = "typedef map<long, string> Lookup; \
               struct TM { Lookup table; long tail; };";
    let body = r#"
        var s = new TM {
            Table = new Lookup(new System.Collections.Generic.Dictionary<int, string> { { 1, "a" }, { 2, "bb" } }),
            Tail = 7,
        };
        var bytes = TMTypeSupport.Instance.Encode(s);
        var back = TMTypeSupport.Instance.Decode(bytes);
        if (back.Table.Value.Count != 2) throw new Exception("typedef-map count");
        if (back.Table.Value[1] != "a" || back.Table.Value[2] != "bb") throw new Exception("typedef-map values");
        if (back.Tail != 7) throw new Exception("trailing scalar after typedef-map");
        Console.WriteLine("OK");
"#;
    assert_roundtrip_ok(idl, body);
}

// ───────────────────────── EDGE CHECKLIST ─────────────────────────
// Adversarial edges that break adapters once they leave hello-world. Each is a
// REAL encode→decode roundtrip through the generated TypeSupport + the actual
// ZeroDDS.Cdr runtime.

#[test]
fn edge_empty_collections() {
    // 1. EMPTY collections: empty unbounded seq, empty bounded seq, empty
    //    string, empty wstring, empty map. count=0 must round-trip, not crash.
    let idl = "@final struct E { \
                 sequence<long> us; \
                 sequence<long, 4> bs; \
                 string s; \
                 wstring w; \
                 map<long, string> m; \
                 long tail; \
               };";
    let body = r#"
        var s = new E {
            Us = new Omg.Types.SequenceList<int>(),
            Bs = new Omg.Types.BoundedList<int>(4),
            S = "",
            W = "",
            M = new System.Collections.Generic.Dictionary<int, string>(),
            Tail = 123,
        };
        var bytes = ETypeSupport.Instance.Encode(s);
        var back = ETypeSupport.Instance.Decode(bytes);
        if (back.Us.Count != 0) throw new Exception("empty unbounded seq count != 0");
        if (back.Bs.Count != 0) throw new Exception("empty bounded seq count != 0");
        if (back.S != "") throw new Exception("empty string mangled: '" + back.S + "'");
        if (back.W != "") throw new Exception("empty wstring mangled: '" + back.W + "'");
        if (back.M.Count != 0) throw new Exception("empty map count != 0");
        if (back.Tail != 123) throw new Exception("trailing scalar after empty collections");
        Console.WriteLine("OK");
"#;
    assert_roundtrip_ok(idl, body);
}

#[test]
fn edge_bound_enforcement_over_n_throws() {
    // 2. BOUND enforcement: bounded sequence/string filled exactly to N (ok) and
    //    OVER N (must throw on encode, not silently corrupt). `BoundedList<T>(N)`
    //    is constructed with a LARGER capacity so the container itself accepts the
    //    extra elements — the GENERATED ENCODER is the gate that must reject them.
    let idl = "@final struct Cap { sequence<long, 3> data; string<4> name; long tail; };";
    let body = r#"
        // Exactly N: ok.
        var ok = new Cap {
            Data = new Omg.Types.BoundedList<int>(3) { 1, 2, 3 },
            Name = "abcd",
            Tail = 9,
        };
        var back = CapTypeSupport.Instance.Decode(CapTypeSupport.Instance.Encode(ok));
        if (back.Data.Count != 3 || back.Data[2] != 3) throw new Exception("exact-N seq");
        if (back.Name != "abcd") throw new Exception("exact-N string");
        if (back.Tail != 9) throw new Exception("tail after exact-N");

        // Over N (sequence): a 4-element list in a bound-3 member must throw on
        // encode. (Construct the container with capacity 10 so the list accepts
        // the 4th element; the encoder's bound check is what must reject it.)
        bool seqThrew = false;
        var overSeq = new Cap {
            Data = new Omg.Types.BoundedList<int>(10) { 1, 2, 3, 4 },
            Name = "ab",
            Tail = 0,
        };
        try { CapTypeSupport.Instance.Encode(overSeq); }
        catch (Exception) { seqThrew = true; }
        if (!seqThrew) throw new Exception("over-bound sequence<long,3> did NOT throw on encode");

        // Over N (string): UTF-8 byte length > 4 must throw.
        bool strThrew = false;
        var overStr = new Cap {
            Data = new Omg.Types.BoundedList<int>(3) { 1 },
            Name = "abcde",
            Tail = 0,
        };
        try { CapTypeSupport.Instance.Encode(overStr); }
        catch (Exception) { strThrew = true; }
        if (!strThrew) throw new Exception("over-bound string<4> did NOT throw on encode");
        Console.WriteLine("OK");
"#;
    assert_roundtrip_ok(idl, body);
}

#[test]
fn edge_deep_nesting() {
    // 3. DEEP nesting: struct→struct→struct (3 levels); sequence<sequence<struct>>;
    //    map<string, struct-with-a-sequence>.
    let idl = "struct L3 { long v; sequence<long> tags; }; \
               struct L2 { L3 inner; long y; }; \
               struct L1 { L2 mid; long x; \
                           sequence<sequence<L3>> grid; \
                           map<string, L3> byName; };";
    let body = r#"
        var s = new L1 {
            Mid = new L2 { Inner = new L3 { V = 7, Tags = new Omg.Types.SequenceList<int> { 1, 2 } }, Y = 8 },
            X = 9,
            Grid = new Omg.Types.SequenceList<Omg.Types.ISequence<L3>> {
                new Omg.Types.SequenceList<L3> { new L3 { V = 10, Tags = new Omg.Types.SequenceList<int> { 3 } } },
                new Omg.Types.SequenceList<L3> { new L3 { V = 11, Tags = new Omg.Types.SequenceList<int>() }, new L3 { V = 12, Tags = new Omg.Types.SequenceList<int> { 4, 5 } } },
            },
            ByName = new System.Collections.Generic.Dictionary<string, L3> {
                { "a", new L3 { V = 20, Tags = new Omg.Types.SequenceList<int> { 6 } } },
                { "bb", new L3 { V = 21, Tags = new Omg.Types.SequenceList<int>() } },
            },
        };
        var back = L1TypeSupport.Instance.Decode(L1TypeSupport.Instance.Encode(s));
        if (back.Mid.Inner.V != 7 || back.Mid.Inner.Tags.Count != 2 || back.Mid.Inner.Tags[1] != 2) throw new Exception("3-level struct");
        if (back.Mid.Y != 8 || back.X != 9) throw new Exception("3-level scalars");
        if (back.Grid.Count != 2) throw new Exception("seq<seq<struct>> outer");
        if (back.Grid[0][0].V != 10 || back.Grid[1].Count != 2 || back.Grid[1][1].V != 12 || back.Grid[1][1].Tags[1] != 5) throw new Exception("seq<seq<struct>> deep");
        if (back.ByName.Count != 2 || back.ByName["a"].V != 20 || back.ByName["a"].Tags[0] != 6) throw new Exception("map<string,struct-with-seq>");
        if (back.ByName["bb"].Tags.Count != 0) throw new Exception("map value empty seq");
        Console.WriteLine("OK");
"#;
    assert_roundtrip_ok(idl, body);
}

#[test]
fn edge_optional_aggregates_present_and_absent() {
    // 4. @optional of an AGGREGATE: optional sequence, optional nested struct,
    //    optional map, optional string — present AND absent both round-trip
    //    (absent stays null, not a zero-value). This is the `.Value`-on-reference
    //    bug: the encode path emitted `sample.Prop.Value` for reference types.
    let idl = "struct Inner { long a; long b; }; \
               @final struct Opt { \
                 @optional string note; \
                 @optional sequence<long> tags; \
                 @optional Inner nested; \
                 @optional map<long, string> kv; \
                 long tail; \
               };";
    let body = r#"
        // Present.
        var pres = new Opt {
            Note = "hi",
            Tags = new Omg.Types.SequenceList<int> { 1, 2, 3 },
            Nested = new Inner { A = 4, B = 5 },
            Kv = new System.Collections.Generic.Dictionary<int, string> { { 9, "z" } },
            Tail = 100,
        };
        var rp = OptTypeSupport.Instance.Decode(OptTypeSupport.Instance.Encode(pres));
        if (rp.Note != "hi") throw new Exception("present optional string");
        if (rp.Tags is null || rp.Tags.Count != 3 || rp.Tags[2] != 3) throw new Exception("present optional seq");
        if (rp.Nested is null || rp.Nested.A != 4 || rp.Nested.B != 5) throw new Exception("present optional struct");
        if (rp.Kv is null || rp.Kv[9] != "z") throw new Exception("present optional map");
        if (rp.Tail != 100) throw new Exception("tail after present optionals");

        // Absent → all null.
        var abs = new Opt { Note = null, Tags = null, Nested = null, Kv = null, Tail = 200 };
        var ra = OptTypeSupport.Instance.Decode(OptTypeSupport.Instance.Encode(abs));
        if (ra.Note != null) throw new Exception("absent optional string must be null");
        if (ra.Tags != null) throw new Exception("absent optional seq must be null");
        if (ra.Nested != null) throw new Exception("absent optional struct must be null");
        if (ra.Kv != null) throw new Exception("absent optional map must be null");
        if (ra.Tail != 200) throw new Exception("tail after absent optionals");
        Console.WriteLine("OK");
"#;
    assert_roundtrip_ok(idl, body);
}

#[test]
fn edge_unicode_string_and_wstring() {
    // 6. UNICODE: multi-byte UTF-8 (CJK + emoji) in string and UTF-16 in wstring;
    //    exact code points must survive.
    let idl = "@final struct Uni { string s; wstring w; long tail; };";
    let body = r#"
        // CJK 世界 + emoji 🚀 (surrogate pair in UTF-16).
        var s = new Uni { S = "héllo 世界 🚀", W = "wide 世界 🚀", Tail = 1 };
        var back = UniTypeSupport.Instance.Decode(UniTypeSupport.Instance.Encode(s));
        if (back.S != "héllo 世界 🚀") throw new Exception("UTF-8 string code points lost: '" + back.S + "'");
        if (back.W != "wide 世界 🚀") throw new Exception("UTF-16 wstring code points lost: '" + back.W + "'");
        if (back.Tail != 1) throw new Exception("tail after unicode");
        Console.WriteLine("OK");
"#;
    assert_roundtrip_ok(idl, body);
}

#[test]
fn edge_array_of_struct_and_multidim_and_bounded_string() {
    // 7. ARRAY: array-of-struct, multi-dim array, array of bounded-string; every
    //    element distinct so a mis-stride is caught.
    let idl = "struct P { long x; long y; }; \
               @final struct Arr { \
                 P shape[2]; \
                 long grid[2][3]; \
                 string<8> names[3]; \
                 long tail; \
               };";
    let body = r#"
        var s = new Arr {
            Shape = new P[] { new P { X = 1, Y = 2 }, new P { X = 3, Y = 4 } },
            Grid = new int[][] { new int[] { 10, 11, 12 }, new int[] { 13, 14, 15 } },
            Names = new string[] { "a", "bb", "ccc" },
            Tail = 99,
        };
        var back = ArrTypeSupport.Instance.Decode(ArrTypeSupport.Instance.Encode(s));
        if (back.Shape[0].X != 1 || back.Shape[0].Y != 2 || back.Shape[1].X != 3 || back.Shape[1].Y != 4) throw new Exception("array-of-struct mis-stride");
        if (back.Grid[0][0] != 10 || back.Grid[0][2] != 12 || back.Grid[1][0] != 13 || back.Grid[1][2] != 15) throw new Exception("multi-dim array mis-stride");
        if (back.Names[0] != "a" || back.Names[1] != "bb" || back.Names[2] != "ccc") throw new Exception("array-of-bounded-string mis-stride");
        if (back.Tail != 99) throw new Exception("tail after arrays");
        Console.WriteLine("OK");
"#;
    assert_roundtrip_ok(idl, body);
}

#[test]
fn edge_extreme_primitives() {
    // 8. EXTREME primitives: integer min/max, 0, -1; float/double normal values.
    let idl = "@final struct Ext { \
                 short i16; unsigned short u16; \
                 long i32; unsigned long u32; \
                 long long i64; unsigned long long u64; \
                 octet o; float f; double d; \
               };";
    let body = r#"
        var s = new Ext {
            I16 = short.MinValue, U16 = ushort.MaxValue,
            I32 = int.MinValue, U32 = uint.MaxValue,
            I64 = long.MaxValue, U64 = ulong.MaxValue,
            O = (byte)255, F = 3.5f, D = -2.25,
        };
        var back = ExtTypeSupport.Instance.Decode(ExtTypeSupport.Instance.Encode(s));
        if (back.I16 != short.MinValue) throw new Exception("i16 min");
        if (back.U16 != ushort.MaxValue) throw new Exception("u16 max");
        if (back.I32 != int.MinValue) throw new Exception("i32 min");
        if (back.U32 != uint.MaxValue) throw new Exception("u32 max");
        if (back.I64 != long.MaxValue) throw new Exception("i64 max");
        if (back.U64 != ulong.MaxValue) throw new Exception("u64 max");
        if (back.O != 255) throw new Exception("octet 255");
        if (back.F != 3.5f) throw new Exception("float");
        if (back.D != -2.25) throw new Exception("double");

        var z = new Ext { I32 = 0, I64 = -1, U32 = 0, U64 = 0, I16 = 0, U16 = 0, O = 0, F = 0f, D = 0.0 };
        var rz = ExtTypeSupport.Instance.Decode(ExtTypeSupport.Instance.Encode(z));
        if (rz.I32 != 0 || rz.I64 != -1) throw new Exception("zero / minus-one");
        Console.WriteLine("OK");
"#;
    assert_roundtrip_ok(idl, body);
}

#[test]
fn edge_keyed_same_key_different_payload() {
    // 9. KEYED: two samples with the same @key and different payload; the key
    //    fields must hash identically (KeyHash) while the payloads still differ.
    let idl =
        "@final struct Sensor { @key long id; @key string region; long reading; string label; };";
    let body = r#"
        var a = new Sensor { Id = 42, Region = "north", Reading = 100, Label = "first" };
        var b = new Sensor { Id = 42, Region = "north", Reading = 999, Label = "second" };

        var ka = SensorTypeSupport.Instance.KeyHash(a);
        var kb = SensorTypeSupport.Instance.KeyHash(b);
        if (ka.Length != kb.Length) throw new Exception("keyhash length differ");
        for (int i = 0; i < ka.Length; i++) if (ka[i] != kb[i]) throw new Exception("same key must hash identically");

        // Different key → different hash (sanity).
        var c = new Sensor { Id = 43, Region = "north", Reading = 100, Label = "first" };
        var kc = SensorTypeSupport.Instance.KeyHash(c);
        bool diff = false;
        for (int i = 0; i < ka.Length; i++) if (ka[i] != kc[i]) diff = true;
        if (!diff) throw new Exception("different key must hash differently");

        // Payloads round-trip independently.
        var ra = SensorTypeSupport.Instance.Decode(SensorTypeSupport.Instance.Encode(a));
        var rb = SensorTypeSupport.Instance.Decode(SensorTypeSupport.Instance.Encode(b));
        if (ra.Id != 42 || ra.Region != "north" || ra.Reading != 100 || ra.Label != "first") throw new Exception("sample a payload");
        if (rb.Id != 42 || rb.Region != "north" || rb.Reading != 999 || rb.Label != "second") throw new Exception("sample b payload");
        Console.WriteLine("OK");
"#;
    assert_roundtrip_ok(idl, body);
}

#[test]
fn edge_union_default_and_as_seq_and_map_value() {
    // 5. UNION: default branch + union as a sequence element and as a map value.
    //    (Per-branch round-trip is covered by `roundtrip_union`; this adds the
    //    container-embedding + an explicit default-discriminator hit.)
    let idl = "union U switch (long) { case 1: long a; case 2: double b; default: octet c; }; \
               struct Holder { sequence<U> items; map<long, U> byKey; U single; long tail; };";
    let body = r#"
        var u1 = new U { Discriminator = 1, Value = (int)11 };
        var u2 = new U { Discriminator = 2, Value = (double)2.5 };
        var ud = new U { Discriminator = 7, Value = (byte)200 }; // hits default
        var s = new Holder {
            Items = new Omg.Types.SequenceList<U> { u1, ud },
            ByKey = new System.Collections.Generic.Dictionary<int, U> { { 5, u2 }, { 6, ud } },
            Single = ud,
            Tail = 33,
        };
        var back = HolderTypeSupport.Instance.Decode(HolderTypeSupport.Instance.Encode(s));
        if (back.Items.Count != 2) throw new Exception("union-in-seq count");
        if (back.Items[0].Discriminator != 1 || (int)back.Items[0].Value! != 11) throw new Exception("union-in-seq[0]");
        if (back.Items[1].Discriminator != 7 || (byte)back.Items[1].Value! != 200) throw new Exception("union-in-seq default");
        if (back.ByKey[5].Discriminator != 2 || (double)back.ByKey[5].Value! != 2.5) throw new Exception("union-in-map");
        if (back.ByKey[6].Discriminator != 7 || (byte)back.ByKey[6].Value! != 200) throw new Exception("union-in-map default");
        if (back.Single.Discriminator != 7 || (byte)back.Single.Value! != 200) throw new Exception("union default member");
        if (back.Tail != 33) throw new Exception("tail after unions");
        Console.WriteLine("OK");
"#;
    assert_roundtrip_ok(idl, body);
}

#[test]
fn roundtrip_bounded_sequence_member() {
    // CS2 (3): a bounded `sequence<T, N>` MEMBER must decode into a container
    // assignable to its `IBoundedSequence<T>` property. Before the fix the
    // decode local/instantiation was `ISequence<T>` / `SequenceList<T>`, which
    // does NOT implement `IBoundedSequence<T>` — the generated object
    // initializer `Vals = __m0!` failed to compile. Now it decodes into a
    // `BoundedList<T>(N)`.
    let idl = "struct BS { sequence<long, 4> vals; long tail; };";
    let body = r#"
        var s = new BS {
            Vals = new Omg.Types.BoundedList<int>(4) { 1, 2, 3 },
            Tail = 55,
        };
        var bytes = BSTypeSupport.Instance.Encode(s);
        var back = BSTypeSupport.Instance.Decode(bytes);
        if (back.Vals.Count != 3) throw new Exception("bounded-seq count");
        if (back.Vals[0] != 1 || back.Vals[1] != 2 || back.Vals[2] != 3) throw new Exception("bounded-seq values");
        if (back.Vals.Bound != 4) throw new Exception("bounded-seq bound preserved");
        if (back.Tail != 55) throw new Exception("trailing scalar after bounded-seq");
        Console.WriteLine("OK");
"#;
    assert_roundtrip_ok(idl, body);
}

/// Prints the encoded bytes of `main_body` (which must `Console.WriteLine` a
/// hex string) and returns the last whitespace-delimited token (the hex).
fn run_hex(idl: &str, main_body: &str) -> Option<String> {
    run_roundtrip(idl, main_body).map(|s| s.split_whitespace().last().unwrap_or("").to_string())
}

// ---------------------------------------------------------------------------
// P0.3 struct inheritance — base-class fields on the wire.
// Regression: the codec (collect_member_info) iterated only `s.members`, never
// `s.base`, dropping inherited fields. XTypes 1.3 §7.4.3.4.1: base BEFORE
// derived. Byte vectors are the SAME as idl-cpp's (cross-binding identity).
// ---------------------------------------------------------------------------

#[test]
fn inheritance_base_field_before_derived_wire() {
    let idl = "@final struct Base { long a; }; @final struct Derived : Base { long b; };";
    let body = r#"
        var s = new Derived { A = 0x11223344, B = 0x55667788 };
        var bytes = DerivedTypeSupport.Instance.Encode(s);
        Console.WriteLine(Convert.ToHexString(bytes));
"#;
    let Some(hex) = run_hex(idl, body) else {
        return;
    };
    // a=0x11223344 (LE 44332211) BEFORE b=0x55667788 (LE 88776655); 8 bytes.
    assert_eq!(
        hex, "4433221188776655",
        "base field `a` must precede derived `b` on the wire"
    );
}

#[test]
fn inheritance_multilevel_base_order_wire() {
    // Type names avoid the C# CS0542 clash (a PascalCased member `b` would equal
    // an enclosing type `B`); the wire vector is name-independent.
    let idl = "@final struct Lvl0 { long a; }; @final struct Lvl1 : Lvl0 { long b; }; \
               @final struct Lvl2 : Lvl1 { long c; };";
    let body = r#"
        var s = new Lvl2 { A = 0x11111111, B = 0x22222222, C = 0x33333333 };
        var bytes = Lvl2TypeSupport.Instance.Encode(s);
        Console.WriteLine(Convert.ToHexString(bytes));
"#;
    let Some(hex) = run_hex(idl, body) else {
        return;
    };
    assert_eq!(hex, "111111112222222233333333");
}

#[test]
fn inheritance_roundtrip_recovers_base_and_derived() {
    let idl = "@final struct Base { long a; }; @final struct Derived : Base { long b; };";
    let body = r#"
        var s = new Derived { A = 0x11223344, B = 0x55667788 };
        var bytes = DerivedTypeSupport.Instance.Encode(s);
        var back = DerivedTypeSupport.Instance.Decode(bytes);
        if (back.A != 0x11223344) throw new Exception("base field a lost on decode");
        if (back.B != 0x55667788) throw new Exception("derived field b lost on decode");
        Console.WriteLine("OK");
"#;
    assert_roundtrip_ok(idl, body);
}

#[test]
fn inheritance_keyhash_includes_base_key() {
    let idl = "@final struct Base { @key long a; }; \
               @final struct Derived : Base { @key long b; };";
    let body = r#"
        var s = new Derived { A = 0x0000000A, B = 0x0000000B };
        var h = DerivedTypeSupport.Instance.KeyHash(s);
        Console.WriteLine(Convert.ToHexString(h));
"#;
    let Some(hex) = run_hex(idl, body) else {
        return;
    };
    // BE key: base a=0x0A then derived b=0x0B, 8-byte holder zero-padded to 16.
    assert_eq!(hex, "0000000A0000000B0000000000000000");
}
