// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Adversarial corpus — generated Python is fed to a REAL Python interpreter
//! and must byte-compile (syntax-valid). Gated on `python3` being on `PATH`;
//! when it is absent (e.g. a minimal CI image) the tests self-skip loudly.
//!
//! Three corpora, per the IDL-construct fix-campaign test plan:
//!   1. reserved-keyword — every Python keyword usable as an IDL identifier,
//!      placed at member / struct / enum / module / const / union-branch
//!      positions (`import`/`in`/`case` are IDL keywords, so they never reach
//!      codegen and are skipped when the IDL fails to parse).
//!   2. construct — each IDL construct minimally, compiled, with codegen-level
//!      width proxies asserted (`WChar` = 2-byte holder, `LongDouble` = 16-byte,
//!      `Fixed`, `Bitset`, bounded seq/string) where the runtime is not present
//!      to measure the wire directly.
//!   3. compose — two IDLs generated separately and idiomatically merged into
//!      one module, compiled together.
//!
//! Note on wire sizes: the `zerodds` Python runtime is not importable in the
//! cargo-test environment, so the corpus checks SYNTAX (`py_compile`) plus the
//! emitted width-bearing brand tokens, not live encode roundtrips.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr
)]

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_python::{PythonGenOptions, generate_python_module};

/// Locates a usable `python3`; `None` (→ skip) when absent.
fn python3() -> Option<String> {
    for bin in ["python3", "python"] {
        if Command::new(bin)
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
        {
            return Some(bin.to_string());
        }
    }
    None
}

/// Generates the Python module for `src`, or `None` if the IDL does not parse
/// (used to skip keyword positions that collide with IDL keywords).
fn try_emit(src: &str) -> Option<String> {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).ok()?;
    generate_python_module(&ast, &PythonGenOptions::default()).ok()
}

fn emit(src: &str) -> String {
    try_emit(src).unwrap_or_else(|| panic!("emit failed for: {src}"))
}

/// A unique scratch directory for this test run.
fn scratch(tag: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "zerodds_idl_python_corpus_{tag}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("mkdir scratch");
    dir
}

/// Byte-compiles every `.py` in `dir` with `python3 -m py_compile`. Panics with
/// the interpreter's diagnostics if any file has a syntax error.
fn compile_dir(py: &str, dir: &Path) {
    let files: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("read scratch")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "py"))
        .collect();
    assert!(!files.is_empty(), "no modules written to {}", dir.display());
    let mut cmd = Command::new(py);
    cmd.arg("-m").arg("py_compile");
    for f in &files {
        cmd.arg(f);
    }
    let out = cmd.output().expect("run py_compile");
    assert!(
        out.status.success(),
        "py_compile failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn write_module(dir: &Path, name: &str, source: &str) {
    std::fs::write(dir.join(format!("{name}.py")), source).expect("write module");
}

const PYTHON_KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue",
    "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import",
    "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while",
    "with", "yield", "match", "case",
];

#[test]
fn reserved_keyword_corpus_compiles() {
    let Some(py) = python3() else {
        eprintln!("SKIP reserved_keyword_corpus: python3 not found");
        return;
    };
    let dir = scratch("kw");
    let mut written = 0usize;
    for (i, kw) in PYTHON_KEYWORDS.iter().enumerate() {
        // Each position, as a separate top-level construct. `_x` avoids a
        // secondary collision when the keyword itself is used as the enclosing
        // type name.
        let positions = [
            format!("struct Smem_{i} {{ long {kw}; boolean b; }};"),
            format!("struct {kw} {{ long v; }};"),
            format!("enum Eenum_{i} {{ {kw}, other_{i} }};"),
            format!("module {kw} {{ struct Inner_{i} {{ long v; }}; }};"),
            format!("const long {kw} = {i};"),
            format!("union Uun_{i} switch(long) {{ case 0: long {kw}; default: long d; }};"),
        ];
        for (p, src) in positions.iter().enumerate() {
            // Skip IDL-keyword collisions (import/in/case) that do not parse.
            if let Some(module) = try_emit(src) {
                write_module(&dir, &format!("kw_{i}_{p}"), &module);
                written += 1;
            }
        }
    }
    assert!(
        written > 100,
        "expected a broad keyword corpus, got {written}"
    );
    compile_dir(&py, &dir);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn construct_corpus_compiles_with_width_proxies() {
    let Some(py) = python3() else {
        eprintln!("SKIP construct_corpus: python3 not found");
        return;
    };
    let dir = scratch("construct");

    // Each entry: (module name, IDL, &[expected width-bearing tokens]).
    let cases: &[(&str, &str, &[&str])] = &[
        // wchar → 2-byte holder brand.
        ("wchar_c", "struct S { wchar w; };", &["WChar"]),
        // long double → 16-byte holder brand.
        (
            "longdouble_c",
            "struct S { long double ld; };",
            &["LongDouble"],
        ),
        // fixed<P,S>.
        (
            "fixed_c",
            "struct S { fixed<10,2> money; };",
            &["Fixed[10, 2]"],
        ),
        // enum with @value gaps.
        (
            "enum_value_c",
            "enum E { A, @value(7) B, C };",
            &["A = 0", "B = 7", "C = 8"],
        ),
        // const of every scalar type.
        (
            "const_all_c",
            "const short CS = -3; const long CL = 10; const long long CLL = 99; \
             const unsigned long CUL = 7; const float CF = 1.5; const double CD = 2.5; \
             const boolean CB = TRUE; const octet CO = 255; const char CC = 'z'; \
             const string CST = \"hi\"; const wstring CW = L\"wide\"; const wchar CWC = L'q';",
            &["CB = True", "CW = \"wide\"", "CWC = 'q'", "CC = 'z'"],
        ),
        // struct inheritance.
        (
            "inherit_c",
            "struct Base { long a; }; struct Derived : Base { long b; };",
            &["class Derived(Base):"],
        ),
        // union with each discriminator kind.
        (
            "union_disc_c",
            "enum K { K0, K1 }; \
             union Ui switch(long)    { case 0: long i; default: long di; }; \
             union Uc switch(char)    { case 'A': long c; }; \
             union Ub switch(boolean) { case TRUE: long t; case FALSE: long f; }; \
             union Ue switch(K)       { case K1: long e; }; \
             union Uo switch(octet)   { case 5: long o; }; \
             union Us switch(short)   { case -1: long s; };",
            &["65: (\"c\"", "1: (\"t\"", "0: (\"f\"", "1: (\"e\""],
        ),
        // bitset + bitmask.
        (
            "bitset_c",
            "bitset Flags { bitfield<3> lo; bitfield<5> hi; };",
            &["Bitset[8]", "LO_SHIFT", "HI_SHIFT"],
        ),
        (
            "bitmask_c",
            "bitmask Perms { READ, WRITE, EXEC };",
            &["READ = 1 << 0", "_idl_bit_bound = 32"],
        ),
        // @optional + @extensibility.
        (
            "optional_c",
            "@mutable struct S { @optional long maybe; @id(5) long tagged; };",
            &["Optional[Int32]", "extensibility=\"mutable\""],
        ),
        // bounded sequence + string (width-bearing brands).
        (
            "bounded_c",
            "struct S { sequence<long, 4> s; string<8> name; wstring<8> wname; };",
            &[
                "Sequence[Int32, 4]",
                "BoundedString[8]",
                "BoundedWString[8]",
            ],
        ),
        // multidimensional + typedef arrays.
        (
            "array_c",
            "struct S { long grid[3][4]; }; typedef long Row[8];",
            &["Array[Array[Int32, 4], 3]", "Array[Int32, 8]"],
        ),
        // map (bounded + unbounded).
        (
            "map_c",
            "struct S { map<long, string> m; map<long, long, 4> bm; };",
            &["Dict[Int32, String]", "Map[Int32, Int32, 4]"],
        ),
        // nested + reopened module.
        (
            "module_c",
            "module a { module b { struct C { long x; }; }; }; \
             module a { struct D { long y; }; };",
            &["class a_b_C:", "class a_D:"],
        ),
    ];

    for (name, src, tokens) in cases {
        let module = emit(src);
        for tok in *tokens {
            assert!(
                module.contains(tok),
                "construct `{name}` missing token `{tok}`:\n{module}"
            );
        }
        write_module(&dir, name, &module);
    }
    compile_dir(&py, &dir);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn compose_multifile_merges_and_compiles() {
    let Some(py) = python3() else {
        eprintln!("SKIP compose_multifile: python3 not found");
        return;
    };
    let dir = scratch("compose");

    // Two IDL files generated independently — as `idlc` would for a two-file
    // project. File B references a type declared in file A.
    let file_a = emit(
        "module geo { \
            struct Point { double x; double y; }; \
            enum Unit { METER, FOOT }; \
        };",
    );
    let file_b = emit(
        "module shapes { \
            struct Circle { double radius; long id; }; \
            union Shape switch(long) { case 0: long a; case 1: string b; }; \
        };",
    );

    // Idiomatic merge: keep file A whole; append file B's body with its leading
    // comment/import header stripped (Python tolerates duplicate imports, but a
    // clean single import block is idiomatic). The two files declare disjoint
    // symbols, so no rebinding occurs.
    let merged = merge_modules(&file_a, &file_b);
    // Sanity: both types survive the merge (no symbol lost).
    assert!(merged.contains("class geo_Point:"), "{merged}");
    assert!(merged.contains("class shapes_Circle:"), "{merged}");
    assert!(merged.contains("shapes_Shape = idl_union("), "{merged}");

    write_module(&dir, "merged", &merged);
    compile_dir(&py, &dir);
    std::fs::remove_dir_all(&dir).ok();
}

/// Merges two generated modules into one compilable source the way a project
/// combining two codegen outputs would: one header, the UNION of both files'
/// import lines (deduped so every referenced name resolves), then both bodies.
fn merge_modules(a: &str, b: &str) -> String {
    let is_import = |t: &str| t.starts_with("from ") || t.starts_with("import ");
    let mut imports: Vec<String> = Vec::new();
    let mut body = String::new();
    for src in [a, b] {
        for line in src.lines() {
            let t = line.trim_start();
            if t.starts_with('#') {
                continue; // drop per-file SPDX/banner comments
            }
            if is_import(t) {
                if !imports.iter().any(|i| i == line) {
                    imports.push(line.to_string());
                }
            } else {
                body.push_str(line);
                body.push('\n');
            }
        }
    }
    let mut out = String::from("# SPDX-License-Identifier: Apache-2.0\n");
    for imp in &imports {
        out.push_str(imp);
        out.push('\n');
    }
    out.push('\n');
    out.push_str(&body);
    out
}
