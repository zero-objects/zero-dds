//! TS-3 — Codegen-Compile-Tests fuer C++.
//!
//! Wir generieren einen C++17-Header aus einem IDL-Snippet und
//! reichen ihn an `clang++ -fsyntax-only -std=c++17` weiter.
//! Compile-Errors werden als Test-Fail propagiert.
//!
//! **Voraussetzung:** `clang++` oder `g++` im `PATH`. Tests werden
//! geskippt wenn weder noch verfuegbar ist (CI-Image-flexibel).

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
    clippy::approx_constant,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::process::Command;

use tempfile::NamedTempFile;
use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CppGenOptions, generate_cpp_header};

fn cpp_compiler() -> Option<&'static str> {
    ["clang++", "g++"].into_iter().find(|cc| {
        Command::new(cc)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .ok()
            .filter(|s| s.success())
            .is_some()
    })
}

/// Schreibt den emittierten Header in eine temporaere `.hpp`-Datei
/// und ruft `clang++ -fsyntax-only`. Liefert Compile-Output bei
/// Fail; `Ok(())` bei Erfolg.
fn check_compiles(cpp_source: &str) -> Result<(), String> {
    let Some(cc) = cpp_compiler() else {
        eprintln!("WARNING: skipping C++ compile-check, no compiler in PATH");
        return Ok(());
    };

    use std::io::Write;
    let mut header = NamedTempFile::with_suffix(".hpp").map_err(|e| e.to_string())?;
    // Vorab Standard-Header (cstdint etc) injizieren ist unnoetig —
    // das Codegen-Output enthaelt bereits seine eigenen Includes.
    header
        .write_all(cpp_source.as_bytes())
        .map_err(|e| e.to_string())?;

    let mut tu = NamedTempFile::with_suffix(".cpp").map_err(|e| e.to_string())?;
    writeln!(tu, "#include \"{}\"", header.path().display()).map_err(|e| e.to_string())?;

    // `dds/topic/TopicTraits.hpp` lebt im `crates/cpp/include`-Tree,
    // sodass die emittierten `topic_type_support<T>`-Spezialisierungen
    // gegen die `cdr_lite`-Helpers compile-checked werden koennen.
    let cpp_include = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("cpp")
        .join("include");
    let include_arg = format!("-I{}", cpp_include.display());

    let output = Command::new(cc)
        .args(["-std=c++17", "-fsyntax-only", "-Wall", "-Wno-unused"])
        .arg(&include_arg)
        .arg(tu.path())
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "compile FAILED with {cc}:\n--- header ---\n{cpp_source}\n--- stderr ---\n{stderr}"
        ))
    }
}

fn gen_default(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_cpp_header(&ast, &CppGenOptions::default()).expect("gen")
}

#[test]
fn compiles_simple_struct() {
    let cpp = gen_default("struct Point { long x; long y; };");
    check_compiles(&cpp).expect("simple struct must compile");
}

#[test]
fn compiles_struct_with_string_sequence() {
    let cpp =
        gen_default("struct Bag { string name; sequence<long> ids; sequence<string, 16> tags; };");
    check_compiles(&cpp).expect("struct with string+seq must compile");
}

#[test]
fn compiles_module_nesting() {
    let cpp = gen_default("module Outer { module Inner { struct S { long x; }; }; };");
    check_compiles(&cpp).expect("nested modules must compile");
}

#[test]
fn compiles_enum() {
    let cpp = gen_default("enum Color { RED, GREEN, BLUE };");
    check_compiles(&cpp).expect("enum must compile");
}

#[test]
fn compiles_typedef() {
    let cpp = gen_default("typedef long Counter;");
    check_compiles(&cpp).expect("typedef must compile");
}

#[test]
fn compiles_inheritance() {
    let cpp =
        gen_default("struct Base { long base_field; }; struct Child : Base { long child_field; };");
    check_compiles(&cpp).expect("inheritance must compile");
}

#[test]
fn compiles_keyed_struct() {
    let cpp = gen_default("struct Sensor { @key long id; double value; };");
    check_compiles(&cpp).expect("keyed struct must compile");
}

#[test]
fn compiles_optional_member() {
    let cpp = gen_default("struct S { @optional long maybe; };");
    check_compiles(&cpp).expect("optional must compile");
}

#[test]
fn compiles_union() {
    let cpp = gen_default(
        "union U switch (long) { case 1: long a; case 2: double b; default: octet c; };",
    );
    check_compiles(&cpp).expect("union must compile");
}

#[test]
fn compiles_array_member() {
    let cpp = gen_default("struct M { long matrix[3][4]; };");
    check_compiles(&cpp).expect("array member must compile");
}

#[test]
fn compiles_exception() {
    let cpp = gen_default("exception NotFound { string what_; };");
    check_compiles(&cpp).expect("exception must compile");
}

#[test]
fn compiles_constants() {
    let cpp = gen_default("const long MAX = 100; const double PI = 3.14;");
    check_compiles(&cpp).expect("constants must compile");
}

/// Kompiliert + linkt + fuehrt einen kleinen C++-Roundtrip-Test aus, der
/// `topic_type_support<T>::encode` ⇆ `decode` gegen den emittierten
/// Header validiert. Damit ist die Lücke real geschlossen — nicht nur
/// "syntaktisch valid", sondern "byte-identisch round-trippable".
fn run_roundtrip(cpp_source: &str, body: &str) -> Result<(), String> {
    let Some(cc) = cpp_compiler() else {
        eprintln!("WARNING: skipping C++ roundtrip-check, no compiler in PATH");
        return Ok(());
    };

    use std::io::Write;
    let mut header = NamedTempFile::with_suffix(".hpp").map_err(|e| e.to_string())?;
    header
        .write_all(cpp_source.as_bytes())
        .map_err(|e| e.to_string())?;

    let mut tu = NamedTempFile::with_suffix(".cpp").map_err(|e| e.to_string())?;
    writeln!(tu, "#include \"{}\"", header.path().display()).map_err(|e| e.to_string())?;
    writeln!(tu, "#include <cassert>").map_err(|e| e.to_string())?;
    writeln!(tu, "int main() {{").map_err(|e| e.to_string())?;
    writeln!(tu, "{body}").map_err(|e| e.to_string())?;
    writeln!(tu, "    return 0;").map_err(|e| e.to_string())?;
    writeln!(tu, "}}").map_err(|e| e.to_string())?;

    let cpp_include = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("cpp")
        .join("include");
    let include_arg = format!("-I{}", cpp_include.display());

    let bin = NamedTempFile::new().map_err(|e| e.to_string())?;
    let bin_path = bin.path().to_path_buf();
    drop(bin);

    let output = Command::new(cc)
        .args(["-std=c++17", "-Wall", "-Wno-unused"])
        .arg(&include_arg)
        .arg(tu.path())
        .arg("-o")
        .arg(&bin_path)
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "compile/link FAILED with {cc}:\n--- header ---\n{cpp_source}\n--- stderr ---\n{stderr}"
        ));
    }

    let run = Command::new(&bin_path)
        .output()
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&bin_path);
    if run.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&run.stderr);
        let stdout = String::from_utf8_lossy(&run.stdout);
        Err(format!(
            "runtime FAILED:\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
        ))
    }
}

#[test]
fn roundtrip_simple_struct() {
    let cpp = gen_default("struct Point { long x; long y; };");
    let body = "    ::Point p;\n\
                p.x(42); p.y(-7);\n\
                auto buf = ::dds::topic::topic_type_support<::Point>::encode(p);\n\
                auto q = ::dds::topic::topic_type_support<::Point>::decode(buf.data(), buf.size());\n\
                assert(q.x() == 42);\n\
                assert(q.y() == -7);\n";
    run_roundtrip(&cpp, body).expect("simple struct roundtrip");
}

#[test]
fn roundtrip_struct_with_string_and_sequence() {
    let cpp =
        gen_default("struct Bag { string name; sequence<long> ids; sequence<string, 16> tags; };");
    let body = r#"
                ::Bag b;
                b.name("hello");
                b.ids(std::vector<int32_t>{1, 2, 3});
                b.tags(std::vector<std::string>{"a", "bc", "def"});
                auto buf = ::dds::topic::topic_type_support<::Bag>::encode(b);
                auto q = ::dds::topic::topic_type_support<::Bag>::decode(buf.data(), buf.size());
                assert(q.name() == "hello");
                assert(q.ids().size() == 3);
                assert(q.ids()[0] == 1 && q.ids()[1] == 2 && q.ids()[2] == 3);
                assert(q.tags().size() == 3);
                assert(q.tags()[0] == "a" && q.tags()[1] == "bc" && q.tags()[2] == "def");
"#;
    run_roundtrip(&cpp, body).expect("string+seq roundtrip");
}

#[test]
fn roundtrip_module_nested() {
    let cpp = gen_default("module Outer { module Inner { struct S { long x; }; }; };");
    let body = "    ::Outer::Inner::S s; s.x(1234);\n\
                auto buf = ::dds::topic::topic_type_support<::Outer::Inner::S>::encode(s);\n\
                auto q = ::dds::topic::topic_type_support<::Outer::Inner::S>::decode(buf.data(), buf.size());\n\
                assert(q.x() == 1234);\n\
                const char* tn = ::dds::topic::topic_type_support<::Outer::Inner::S>::type_name();\n\
                assert(std::string(tn) == \"Outer::Inner::S\");\n";
    run_roundtrip(&cpp, body).expect("nested-module roundtrip");
}

#[test]
fn roundtrip_primitives_and_bool() {
    let cpp = gen_default(
        "struct All {\n\
            boolean b; octet o; short s; unsigned short us;\n\
            long l; unsigned long ul; long long ll; unsigned long long ull;\n\
            float f; double d;\n\
        };",
    );
    let body = r#"
                ::All a;
                a.b(true); a.o(0xAB); a.s(-12345); a.us(54321);
                a.l(-1234567); a.ul(2345678); a.ll(-987654321LL); a.ull(123456789ULL);
                a.f(2.5f); a.d(3.14159);
                auto buf = ::dds::topic::topic_type_support<::All>::encode(a);
                auto q = ::dds::topic::topic_type_support<::All>::decode(buf.data(), buf.size());
                assert(q.b() == true);
                assert(q.o() == 0xAB);
                assert(q.s() == -12345);
                assert(q.us() == 54321);
                assert(q.l() == -1234567);
                assert(q.ul() == 2345678);
                assert(q.ll() == -987654321LL);
                assert(q.ull() == 123456789ULL);
                assert(q.f() == 2.5f);
                assert(q.d() == 3.14159);
"#;
    run_roundtrip(&cpp, body).expect("primitives roundtrip");
}
