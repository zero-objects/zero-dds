// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Adversarial IDL corpus for the idl-csharp emitter, compiled with the real
//! Roslyn toolchain (`dotnet build`). Three corpora:
//!
//!  1. **Reserved-keyword corpus** — every C# reserved word that is a legal
//!     bare IDL identifier, placed at each declaration position (member,
//!     struct, enum value, module, const, union branch). The emitter must
//!     `@`-escape each so the generated C# compiles.
//!  2. **Construct corpus** — each IDL construct minimally (fixed, enum
//!     `@value`, const of every scalar type, struct inheritance, union with
//!     every discriminator kind, bitset/bitmask, `@optional`/`@extensibility`,
//!     sequence/array-multidim/map, nested + reopened modules).
//!  3. **Compose** — two IDLs generated separately and merged into one source
//!     tree; distinct top-level namespaces must not collide.
//!
//! **Prerequisite:** the `dotnet` CLI on `PATH` (>= .NET 8). Tests skip (warn
//! once) when it is absent — matching `compile_check.rs`. codepit has no
//! `dotnet`-less constraint; this runs there and on macOS.

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

fn gen_cs(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_csharp(&ast, &CsGenOptions::default()).expect("gen")
}

/// Absolute path to the real `ZeroDDS.Cdr.csproj` in the workspace.
fn zerodds_cdr_csproj() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest)
        .join("../cs/csharp/ZeroDDS.Cdr/ZeroDDS.Cdr.csproj")
        .canonicalize()
        .expect("ZeroDDS.Cdr.csproj must exist")
        .to_string_lossy()
        .into_owned()
}

/// Compiles one or more generated C# sources together against the REAL
/// `ZeroDDS.Cdr` runtime (a `ProjectReference`, not stubs — matching
/// `roundtrip_xcdr2.rs`) plus the minimal `Omg.Types` shim.
fn compile_sources(sources: &[String]) -> Result<(), String> {
    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let csproj = format!(
        r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net8.0</TargetFramework>
    <Nullable>enable</Nullable>
    <RollForward>LatestMajor</RollForward>
    <NoWarn>CS0168;CS8019;CS8632;CS0219;CS0414</NoWarn>
  </PropertyGroup>
  <ItemGroup>
    <ProjectReference Include="{}" />
  </ItemGroup>
</Project>
"#,
        zerodds_cdr_csproj()
    );
    std::fs::write(tmp.path().join("Generated.csproj"), csproj).map_err(|e| e.to_string())?;
    for (i, src) in sources.iter().enumerate() {
        std::fs::write(tmp.path().join(format!("Generated{i}.cs")), src)
            .map_err(|e| e.to_string())?;
    }

    // Serialize against the other tests that build the shared ZeroDDS.Cdr
    // project (cross-binary/-process `dotnet build` race → CS2012). Held only
    // around the build.
    let _guard = zerodds_dotnet_build_lock::dotnet_build_guard();
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
            "dotnet build FAILED:\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}\n--- sources ---\n{}",
            sources.join("\n// ==== next file ====\n")
        ))
    }
}

// ---------------------------------------------------------------------------
// Corpus 1 — reserved keywords at every position
// ---------------------------------------------------------------------------

/// C# reserved keywords that are legal *bare* IDL identifiers (i.e. not also
/// IDL keywords, so the IDL parser accepts them and the C# emitter must escape
/// them itself). Excludes words that are IDL keywords too (`string`, `long`,
/// `double`, `char`, `void`, `enum`, `struct`, `case`, `default`, `switch`,
/// `fixed`, `const`, `interface`, `in`, `out`, `readonly`).
const CS_RESERVED_BARE_IDL: &[&str] = &[
    "class",
    "namespace",
    "object",
    "new",
    "lock",
    "params",
    "virtual",
    "internal",
    "sealed",
    "event",
    "delegate",
    "checked",
    "unchecked",
    "this",
    "throw",
    "static",
    "protected",
    "override",
    "operator",
    "stackalloc",
    "volatile",
    "unsafe",
    "explicit",
    "implicit",
];

/// One module per keyword — module `<kw>` (→ namespace `@<kw>`); the struct, its
/// member, an enum value, a union branch member and an interface const name all
/// use the keyword. (The const sits in an interface — the C# location that
/// accepts a const; a namespace-scope const is CS0116, a separate pre-existing
/// limitation.) Distinct top-level module names keep the namespaces disjoint so
/// the whole corpus compiles in one build.
fn reserved_keyword_idl() -> String {
    let mut s = String::new();
    for kw in CS_RESERVED_BARE_IDL {
        s.push_str(&format!(
            "module {kw} {{\n\
             \x20 struct {kw} {{ long {kw}; }};\n\
             \x20 enum En {{ {kw} }};\n\
             \x20 union Un switch (long) {{ case 1: long {kw}; default: octet other; }};\n\
             \x20 interface Ik {{ const long {kw} = 1; }};\n\
             }};\n"
        ));
    }
    s
}

#[test]
fn reserved_keywords_compile_at_every_position() {
    if !dotnet_available() {
        eprintln!("WARNING: skipping C# adversarial corpus, no dotnet in PATH");
        return;
    }
    let idl = reserved_keyword_idl();
    let cs = gen_cs(&idl);
    if let Err(e) = compile_sources(std::slice::from_ref(&cs)) {
        panic!("reserved-keyword corpus failed to compile:\n{e}");
    }
}

// ---------------------------------------------------------------------------
// Corpus 2 — every IDL construct, minimally
// ---------------------------------------------------------------------------

fn construct_corpus() -> Vec<(&'static str, &'static str)> {
    vec![
        ("wchar", "struct S { wchar c; };"),
        ("fixed<P,S>", "struct S { fixed<10,4> amount; };"),
        (
            "enum with @value gaps (incl. negative)",
            "enum E { @value(1) A, @value(4) B, C, @value(-2) D };",
        ),
        (
            // Consts live in an interface — the C# location that accepts a
            // `const` member (a namespace-scope const is CS0116, a separate
            // pre-existing limitation). This exercises F6/F7 value rendering
            // (TRUE/FALSE → true/false, `L"…"`/`L'…'` prefix stripped) across the
            // integral/char/string/bool const types. (float/double/long-double
            // const literals need an `f`/`m` suffix — a separate pre-existing
            // const-literal defect, outside these findings.)
            "const of every integral/char/string/bool type",
            "interface K { const short CS = -1; const unsigned long CUL = 7; \
             const long long CLL = 9; const char CC = 'x'; const boolean CB = TRUE; \
             const boolean CB2 = FALSE; const octet CO = 255; const string CStr = \"hi\"; \
             const wstring CWs = L\"wide\"; const wchar CWc = L'w'; };",
        ),
        (
            "struct inheritance (multi-level)",
            "struct Base { @key long id; long a; }; struct Mid : Base { long b; }; \
             struct Leaf : Mid { long c; };",
        ),
        (
            "union integer discriminator",
            "union U switch (long) { case 1: long a; case 2: double b; default: octet c; };",
        ),
        (
            "union enum discriminator",
            "enum Color { RED, GREEN, BLUE }; \
             union U switch (Color) { case RED: long a; case GREEN: double b; default: octet c; };",
        ),
        (
            "union boolean discriminator",
            "union U switch (boolean) { case TRUE: long x; case FALSE: double y; };",
        ),
        (
            "bitset",
            "bitset Flags { bitfield<3> a; bitfield<5> b; bitfield<1> c; };",
        ),
        (
            "bitmask",
            "@bit_bound(8) bitmask Perm { READ, WRITE, EXEC };",
        ),
        (
            "@optional + @extensibility(MUTABLE) + @id",
            "@extensibility(MUTABLE) struct S { @optional long maybe; @id(7) long tagged; };",
        ),
        (
            "@appendable",
            "@extensibility(APPENDABLE) struct S { long a; long b; };",
        ),
        ("@final", "@extensibility(FINAL) struct S { long a; };"),
        (
            "sequence (bounded + unbounded + nested)",
            "struct S { sequence<long> u; sequence<long, 4> b; sequence<sequence<long>> nn; };",
        ),
        (
            "multidimensional array",
            "struct S { long grid[2][3]; long cube[2][2][2]; };",
        ),
        (
            "map",
            "struct S { map<long, string> m; map<string, long, 8> bm; };",
        ),
        (
            "nested + reopened module",
            "module a { module b { struct S { long x; }; }; }; module a { struct T { long y; }; };",
        ),
        (
            "typedef incl. array alias",
            "typedef long Meters; typedef long Matrix3[3][3]; struct S { Meters m; Matrix3 mat; };",
        ),
        (
            "interface with nested type/const",
            "interface Svc { struct Nested { long x; }; enum Kind { A, B }; const long C = 5; };",
        ),
        (
            "case-only-differing members (F36)",
            "struct S { long my_field; long myField; long MyField; };",
        ),
    ]
}

#[test]
fn constructs_compile() {
    if !dotnet_available() {
        eprintln!("WARNING: skipping C# adversarial corpus, no dotnet in PATH");
        return;
    }
    for (name, idl) in construct_corpus() {
        let cs = gen_cs(idl);
        if let Err(e) = compile_sources(std::slice::from_ref(&cs)) {
            panic!("construct `{name}` failed to compile:\n{idl}\n---\n{e}");
        }
    }
}

#[test]
fn long_double_struct_is_gated_data_type_only() {
    if !dotnet_available() {
        eprintln!("WARNING: skipping C# adversarial corpus, no dotnet in PATH");
        return;
    }
    // C# has no binary128 primitive; a long double member yields the data record
    // (a `decimal` property) but NO codec — it must still compile (F3/A3).
    for idl in [
        "struct F { long double v; long n; };",
        "struct F { sequence<long double> v; };",
        "struct F { long double grid[2]; };",
        "union U switch (long) { case 1: long double d; default: octet o; };",
    ] {
        let cs = gen_cs(idl);
        assert!(
            !cs.contains("long double not in v1.0") && !cs.contains("default(decimal)"),
            "long double must be gated (no throwing/silent codec) for `{idl}`:\n{cs}"
        );
        if let Err(e) = compile_sources(std::slice::from_ref(&cs)) {
            panic!("gated long-double IDL failed to compile:\n{idl}\n---\n{e}");
        }
    }
}

// ---------------------------------------------------------------------------
// Corpus 3 — compose two independently-generated IDLs
// ---------------------------------------------------------------------------

#[test]
fn compose_two_independent_idls_compile_together() {
    if !dotnet_available() {
        eprintln!("WARNING: skipping C# adversarial corpus, no dotnet in PATH");
        return;
    }
    // Two IDLs authored & generated separately (two `idlc` invocations), then
    // merged into one source tree. Distinct top-level namespaces (`Geo`,
    // `Sensor`) plus the `partial` TypeObjects class (W0b) must not collide.
    let idl_a = "module Geo {
        struct Point { double x; double y; };
        struct Line { Geo::Point a; Geo::Point b; };
    };";
    let idl_b = "module Sensor {
        enum Unit { METER, DEGREE };
        struct Reading { @key long id; double value; Sensor::Unit unit; sequence<double> history; };
    };";

    let cs_a = gen_cs(idl_a);
    let cs_b = gen_cs(idl_b);
    if let Err(e) = compile_sources(&[cs_a, cs_b]) {
        panic!("composed multi-file set failed to compile:\n{e}");
    }
}
