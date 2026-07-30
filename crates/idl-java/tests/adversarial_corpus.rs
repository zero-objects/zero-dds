//! Gated adversarial corpus for the IDL→Java codegen.
//!
//! Requires a real `javac` on `PATH`; every generated file is compiled
//! against the **real** ZeroDDS Java runtime (java-omgdds + idl-java/runtime),
//! never stubs. When `javac` is absent the tests skip (warn once), mirroring
//! `compile_check.rs`.
//!
//! Three corpora:
//!   1. reserved-keyword — every Java reserved word that is a legal bare IDL
//!      identifier, placed at member / struct / enum-value / module / const /
//!      union-branch positions, generated and compiled. Proves the keyword
//!      sanitizer (`class` -> `class_`) fires at every position.
//!   2. construct — each IDL construct minimally (wchar, fixed, enum @value,
//!      const of every type, struct inheritance, every union discriminator
//!      kind, bitset, bitmask, @optional/@extensibility, sequence, multidim
//!      array, map, nested + reopened module) compiled; `long double` asserts
//!      loud rejection (no Java binary128).
//!   3. compose-multifile — two independent IDLs generated separately, their
//!      Java file-sets merged into one source tree, compiled together (no
//!      cross-file symbol collision).
//!
//! Wire-size behaviour (wchar = 2 bytes, arrays/maps/unions/inheritance/
//! optional round-trips) is proven separately and executably in
//! `typesupport_roundtrip.rs`; this file proves the code compiles.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::uninlined_format_args,
    missing_docs
)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_java::{JavaFile, JavaGenError, JavaGenOptions, generate_java_files};

// ---------------------------------------------------------------------------
// javac harness (mirrors compile_check.rs — real runtime, no stubs)
// ---------------------------------------------------------------------------

fn javac_available() -> bool {
    Command::new("javac")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn collect_java(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_java(&path, out);
        } else if path.extension().is_some_and(|e| e == "java") {
            out.push(path);
        }
    }
}

/// Compiles the real ZeroDDS Java runtime once, returns the classes dir.
fn real_runtime_classes() -> Option<&'static Path> {
    static CELL: OnceLock<Option<(tempfile::TempDir, PathBuf)>> = OnceLock::new();
    CELL.get_or_init(|| {
        if !javac_available() {
            eprintln!("WARNING: skipping Java adversarial corpus, no javac in PATH");
            return None;
        }
        let tmp = tempfile::tempdir().ok()?;
        let out = tmp.path().join("classes");
        std::fs::create_dir_all(&out).ok()?;
        let mut srcs = Vec::new();
        collect_java(
            &manifest().join("../java-omgdds/java/src/main/java"),
            &mut srcs,
        );
        collect_java(&manifest().join("runtime"), &mut srcs);
        assert!(!srcs.is_empty(), "no real runtime sources found");
        let output = Command::new("javac")
            .arg("-nowarn")
            .arg("-d")
            .arg(&out)
            .args(&srcs)
            .output()
            .ok()?;
        assert!(
            output.status.success(),
            "real Java runtime failed to compile:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        Some((tmp, out))
    })
    .as_ref()
    .map(|(_, p)| p.as_path())
}

/// javac-compiles an already-generated Java file-set against the runtime.
fn compile_files(files: &[JavaFile]) -> Result<(), String> {
    let Some(classes) = real_runtime_classes() else {
        return Ok(());
    };
    if files.is_empty() {
        return Ok(());
    }
    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let mut paths = Vec::new();
    for f in files {
        let dir = if f.package_path.is_empty() {
            tmp.path().to_path_buf()
        } else {
            tmp.path().join(f.package_path.replace('.', "/"))
        };
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("{}.java", f.class_name));
        std::fs::write(&path, &f.source).map_err(|e| e.to_string())?;
        paths.push(path);
    }
    let output = Command::new("javac")
        .arg("-nowarn")
        .arg("-classpath")
        .arg(classes)
        .arg("-d")
        .arg(tmp.path().join("out"))
        .args(&paths)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "javac FAILED:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn gen_java(src: &str) -> Result<Vec<JavaFile>, JavaGenError> {
    let ast = zerodds_idl::parse(src, &ParserConfig::default())
        .unwrap_or_else(|e| panic!("parse failed for corpus IDL: {e:?}\n{src}"));
    generate_java_files(&ast, &JavaGenOptions::default())
}

fn compile_idl(src: &str) {
    let files = gen_java(src).unwrap_or_else(|e| panic!("gen failed: {e:?}\n{src}"));
    if let Err(e) = compile_files(&files) {
        panic!("compile failed for IDL:\n{src}\n---\n{e}");
    }
}

// ---------------------------------------------------------------------------
// Corpus 1 — reserved keywords at every position
// ---------------------------------------------------------------------------

/// Java reserved words that are legal *bare* IDL identifiers (i.e. not also
/// IDL keywords, so no `_`-escape is needed — the escape would prefix the
/// name and bypass the very sanitizer under test). Covers real keywords,
/// literals (`true`/`false`/`null`) and restricted identifiers
/// (`record`/`sealed`/`var`/`yield`/`permits`/…).
const JAVA_RESERVED_BARE_IDL: &[&str] = &[
    "assert",
    "break",
    "byte",
    "catch",
    "class",
    "continue",
    "do",
    "else",
    "extends",
    "final",
    "finally",
    "for",
    "goto",
    "if",
    "implements",
    "instanceof",
    "int",
    "new",
    "package",
    "protected",
    "return",
    "static",
    "strictfp",
    "super",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "try",
    "volatile",
    "while",
    "true",
    "false",
    "null",
    "record",
    "sealed",
    "var",
    "yield",
    "permits",
    "exports",
    "open",
    "opens",
    "requires",
    "to",
    "transitive",
    "with",
];

/// Emits one module per keyword. Module `<kw>` -> package `<kw>_`; the struct,
/// its member, an enum value, a union branch and a nested const all use the
/// keyword. Distinct top-level module names keep the packages disjoint, so the
/// whole corpus compiles in a single javac run.
fn reserved_keyword_idl() -> String {
    let mut s = String::new();
    for kw in JAVA_RESERVED_BARE_IDL {
        s.push_str(&format!(
            "module {kw} {{\n\
             \x20 struct {kw} {{ long {kw}; }};\n\
             \x20 enum En {{ {kw} }};\n\
             \x20 union Un switch (long) {{ case 1: long {kw}; default: octet other; }};\n\
             \x20 module kk {{ const long {kw} = 1; }};\n\
             }};\n"
        ));
    }
    s
}

#[test]
fn reserved_keywords_compile_at_every_position() {
    if !javac_available() {
        eprintln!("skipping — no javac");
        return;
    }
    let idl = reserved_keyword_idl();
    // Sanity: generation must succeed for the full corpus.
    let files = gen_java(&idl).expect("reserved-keyword corpus must generate");
    assert!(!files.is_empty());
    compile_idl(&idl);
}

// ---------------------------------------------------------------------------
// Corpus 2 — every IDL construct, minimally
// ---------------------------------------------------------------------------

/// Each construct as its own compilable IDL unit.
fn construct_corpus() -> Vec<(&'static str, &'static str)> {
    vec![
        ("wchar (UTF-16, 2 bytes)", "struct S { wchar c; };"),
        ("fixed<P,S>", "struct S { fixed<10,4> amount; };"),
        (
            "enum with @value gaps",
            "enum E { @value(1) A, @value(4) B, @value(9) C };",
        ),
        (
            "const of every scalar type",
            "const short CS = -1; const unsigned long CUL = 7; const long long CLL = 9; \
             const float CF = 1.5; const double CD = 2.5; const char CC = 'x'; \
             const boolean CB = TRUE; const octet CO = 255; const string CStr = \"hi\"; \
             const wstring CWs = L\"wide\"; const wchar CWc = L'w';",
        ),
        (
            "struct inheritance",
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
            "union char discriminator",
            "union U switch (char) { case 'a': long x; case 'b': double y; default: octet z; };",
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
            "@optional + @extensibility",
            "@extensibility(MUTABLE) struct S { @optional long maybe; @id(7) long tagged; };",
        ),
        (
            "@appendable extensibility",
            "@extensibility(APPENDABLE) struct S { long a; long b; };",
        ),
        (
            "@final extensibility",
            "@extensibility(FINAL) struct S { long a; };",
        ),
        (
            "sequence (bounded + unbounded + nested)",
            "struct S { sequence<long> u; sequence<long, 4> b; sequence<sequence<long>> nn; };",
        ),
        (
            "multidimensional array",
            "struct S { long grid[2][3]; long cube[2][2][2]; };",
        ),
        (
            "array of sequence (generic-array-creation guard)",
            "struct S { sequence<long> rows[3]; };",
        ),
        (
            "map",
            "struct S { map<long, string> m; map<string, long, 8> bm; };",
        ),
        (
            "nested + reopened module",
            "module a { module b { struct S { long x; }; }; }; \
             module a { struct T { long y; }; };",
        ),
        (
            "typedef incl. array alias",
            "typedef long Meters; typedef long Matrix3[3][3]; struct S { Meters m; Matrix3 mat; };",
        ),
        (
            "interface with nested type",
            "interface Svc { struct Nested { long x; }; enum Kind { A, B }; };",
        ),
    ]
}

#[test]
fn constructs_compile() {
    if !javac_available() {
        eprintln!("skipping — no javac");
        return;
    }
    for (name, idl) in construct_corpus() {
        let files = gen_java(idl).unwrap_or_else(|e| panic!("gen failed for {name}: {e:?}"));
        if let Err(e) = compile_files(&files) {
            panic!("construct `{name}` failed to compile:\n{idl}\n---\n{e}");
        }
    }
}

#[test]
fn long_double_is_rejected_not_miscompiled() {
    // 16-byte binary128 has no Java primitive and the runtime no 16-byte
    // float accessor. Every context must reject rather than emit an 8-byte
    // member under a 16-byte length code (F2 / P12).
    for idl in [
        "struct S { long double d; };",
        "struct S { sequence<long double> v; };",
        "struct S { long double grid[2][2]; };",
        "const long double LD = 1.0;",
        "union U switch (long) { case 1: long double d; default: octet o; };",
    ] {
        let r = gen_java(idl);
        assert!(
            matches!(r, Err(JavaGenError::UnsupportedConstruct { .. })),
            "long double must be rejected for `{idl}`, got {r:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Corpus 3 — compose two independently-generated IDLs
// ---------------------------------------------------------------------------

#[test]
fn compose_two_independent_idls_compile_together() {
    if !javac_available() {
        eprintln!("skipping — no javac");
        return;
    }
    // Two IDLs authored & generated separately (as by two `idlc` invocations).
    let idl_a = "module geo {
        struct Point { double x; double y; };
        struct Line { geo::Point a; geo::Point b; };
    };";
    let idl_b = "module sensor {
        enum Unit { METER, DEGREE };
        struct Reading { @key long id; double value; sensor::Unit unit; sequence<double> history; };
    };";

    let mut files = gen_java(idl_a).expect("gen A");
    files.extend(gen_java(idl_b).expect("gen B"));

    // Idiomatic merge = drop both file-sets into one source tree. Distinct
    // top-level packages (`geo`, `sensor`) must not collide.
    let mut seen = std::collections::HashSet::new();
    for f in &files {
        let key = format!("{}/{}", f.package_path, f.class_name);
        assert!(seen.insert(key.clone()), "compose collision: {key}");
    }
    if let Err(e) = compile_files(&files) {
        panic!("composed multi-file set failed to compile:\n{e}");
    }
}
