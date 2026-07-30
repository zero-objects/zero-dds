//! P0-2 regression: C/C++ backends must resolve a member type reference against
//! its own lexical scope, NOT by simple name.
//!
//! Two modules declare the SAME simple names (`Item`, `Scalar`, `Code`) with
//! DIFFERENT shapes/widths. `Alpha::Envelope` must reference `Alpha::Item`,
//! `Alpha::Scalar` (= `long` → int32) and `Alpha::Code` (`@bit_bound(8)` →
//! 1-byte wire); `Beta::Envelope` must reference `Beta::Item`, `Beta::Scalar`
//! (= `double`) and `Beta::Code` (default → 4-byte wire). Before the fix the
//! backend registries were keyed by simple name, so the last-collected module
//! (Beta) silently overwrote Alpha's declarations and `Alpha::Envelope` carried
//! Beta's types/widths.
//!
//! The generated names use the injective `::`→`_s` encoding already on `main`
//! (e.g. `Alpha_sItem_t`), so the C assertions expect that form.
//!
//! String assertions run unconditionally (the decisive guard). Compile checks
//! (`clang++`/`g++`, `clang`/`gcc`) are additionally run when a compiler is on
//! PATH and skipped otherwise (CI-image-flexible).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CGenOptions, CppGenOptions, generate_c_header, generate_cpp_header};

/// Same-named types in two modules, with observably different shapes/widths.
const IDL: &str = r#"
module Alpha {
  @final struct Item { long a; };
  typedef long Scalar;
  @bit_bound(8) enum Code { A_LO, A_HI };
  @appendable struct Envelope { Item i; Scalar s; Code c; };
};
module Beta {
  @final struct Item { double d; };
  typedef double Scalar;
  enum Code { B_LO, B_HI };
  @appendable struct Envelope { Item i; Scalar s; Code c; };
};
"#;

fn gen_cpp() -> String {
    let ast = zerodds_idl::parse(IDL, &ParserConfig::default()).expect("parse");
    generate_cpp_header(&ast, &CppGenOptions::default()).expect("cpp-gen")
}

fn gen_c() -> String {
    let ast = zerodds_idl::parse(IDL, &ParserConfig::default()).expect("parse");
    generate_c_header(&ast, &CGenOptions::default()).expect("c-gen")
}

/// Extract the substring from `start` up to (but not including) `end` — used to
/// scope an assertion to one type's serializer so a match in the OTHER module's
/// code cannot mask a regression.
fn slice_between<'a>(hay: &'a str, start: &str, end: &str) -> &'a str {
    let s = hay
        .find(start)
        .unwrap_or_else(|| panic!("marker not found: {start}"));
    let rest = &hay[s..];
    let e = rest.find(end).map_or(rest.len(), |e| e);
    &rest[..e]
}

// --------------------------------------------------------------------------
// C++ backend
// --------------------------------------------------------------------------

#[test]
fn cpp_envelope_member_types_are_module_local() {
    let cpp = gen_cpp();

    // Field declarations: the class in each module embeds ITS OWN Item.
    assert!(
        cpp.contains("::Alpha::Item i_;"),
        "Alpha::Envelope must declare its member as ::Alpha::Item\n{cpp}"
    );
    assert!(
        cpp.contains("::Beta::Item i_;"),
        "Beta::Envelope must declare its member as ::Beta::Item\n{cpp}"
    );

    // Alpha::Envelope encode: inlines Alpha::Item (field `a`), int32 Scalar,
    // 1-byte (@bit_bound 8) Code holder.
    let alpha = slice_between(
        &cpp,
        "topic_type_support<::Alpha::Envelope>::encode(const ::Alpha::Envelope& zd_v, ",
        "encode_be",
    );
    assert!(
        alpha.contains("zd_v.i().a()"),
        "Alpha::Envelope must inline Alpha::Item's field a()\n{alpha}"
    );
    assert!(
        alpha.contains("write_le_origin<int32_t>(zd_out, zd_origin, zd_v.s()"),
        "Alpha::Scalar must be int32 (long)\n{alpha}"
    );
    assert!(
        alpha.contains("write_le_origin<int8_t>(zd_out, zd_origin, static_cast<int8_t>(zd_v.c())"),
        "Alpha::Code @bit_bound(8) must narrow to int8_t\n{alpha}"
    );
    assert!(
        !alpha.contains(".d()") && !alpha.contains("<double>"),
        "Alpha::Envelope must NOT reference Beta's double-typed members\n{alpha}"
    );

    // Beta::Envelope encode: inlines Beta::Item (field `d`), double Scalar,
    // default (4-byte) Code holder.
    let beta = slice_between(
        &cpp,
        "topic_type_support<::Beta::Envelope>::encode(const ::Beta::Envelope& zd_v, ",
        "encode_be",
    );
    assert!(
        beta.contains("zd_v.i().d()"),
        "Beta::Envelope must inline Beta::Item's field d()\n{beta}"
    );
    assert!(
        beta.contains("write_le_origin<double>(zd_out, zd_origin, zd_v.s()"),
        "Beta::Scalar must be double\n{beta}"
    );
    assert!(
        beta.contains("static_cast<int32_t>(zd_v.c())"),
        "Beta::Code (default bit_bound) must be int32\n{beta}"
    );
}

#[test]
fn cpp_scope_collision_compiles() {
    let cpp = gen_cpp();
    if let Err(e) = cpp_check_compiles(&cpp) {
        panic!("{e}");
    }
}

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

fn cpp_check_compiles(cpp_source: &str) -> Result<(), String> {
    let Some(cc) = cpp_compiler() else {
        eprintln!("WARNING: skipping C++ compile-check, no compiler in PATH");
        return Ok(());
    };
    use std::io::Write;
    let mut header = tempfile::NamedTempFile::with_suffix(".hpp").map_err(|e| e.to_string())?;
    header
        .write_all(cpp_source.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut tu = tempfile::NamedTempFile::with_suffix(".cpp").map_err(|e| e.to_string())?;
    writeln!(tu, "#include \"{}\"", header.path().display()).map_err(|e| e.to_string())?;
    let cpp_include = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("cpp")
        .join("include");
    let output = Command::new(cc)
        .args(["-std=c++17", "-fsyntax-only", "-Wall", "-Wno-unused"])
        .arg(format!("-I{}", cpp_include.display()))
        .arg(tu.path())
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "compile FAILED with {cc}:\n--- header ---\n{cpp_source}\n--- stderr ---\n{}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

// --------------------------------------------------------------------------
// C backend
// --------------------------------------------------------------------------

#[test]
fn c_envelope_member_types_are_module_local() {
    let c = gen_c();

    // Struct field: each Envelope embeds ITS OWN Item (injective `_s` names).
    assert!(
        c.contains("typedef struct Alpha_sEnvelope_s {\n    Alpha_sItem_t i;"),
        "Alpha_Envelope must embed Alpha_sItem_t\n{c}"
    );
    assert!(
        c.contains("typedef struct Beta_sEnvelope_s {\n    Beta_sItem_t i;"),
        "Beta_Envelope must embed Beta_sItem_t\n{c}"
    );

    // Alpha_Envelope encode: Alpha_Item's field `a` (int32), int32 Scalar,
    // 1-byte (@bit_bound 8) Code.
    let alpha = slice_between(
        &c,
        "const Alpha_sEnvelope_t* s = (const Alpha_sEnvelope_t*)sample;",
        "Alpha_sEnvelope_decode",
    );
    assert!(
        alpha.contains("zerodds_xcdr2_c_write_i32(&w_buf, &w_len, &w_cap, (s->i).a)"),
        "Alpha_Envelope must write Alpha_Item's int32 field a\n{alpha}"
    );
    assert!(
        alpha.contains("zerodds_xcdr2_c_write_i32(&w_buf, &w_len, &w_cap, s->s)"),
        "Alpha_Scalar must be int32 (long)\n{alpha}"
    );
    assert!(
        alpha.contains("zerodds_xcdr2_c_write_u8(&w_buf, &w_len, &w_cap, (uint8_t)(s->c))"),
        "Alpha_Code @bit_bound(8) must serialize as a 1-byte holder\n{alpha}"
    );
    assert!(
        !alpha.contains("(s->i).d") && !alpha.contains("write_f64"),
        "Alpha_Envelope must NOT reference Beta's double-typed members\n{alpha}"
    );

    // Beta_Envelope encode: Beta_Item's field `d` (double), double Scalar,
    // default (4-byte) Code.
    let beta = slice_between(
        &c,
        "const Beta_sEnvelope_t* s = (const Beta_sEnvelope_t*)sample;",
        "Beta_sEnvelope_decode",
    );
    assert!(
        beta.contains("zerodds_xcdr2_c_write_f64(&w_buf, &w_len, &w_cap, (s->i).d)"),
        "Beta_Envelope must write Beta_Item's double field d\n{beta}"
    );
    assert!(
        beta.contains("zerodds_xcdr2_c_write_f64(&w_buf, &w_len, &w_cap, s->s)"),
        "Beta_Scalar must be double\n{beta}"
    );
    assert!(
        beta.contains("zerodds_xcdr2_c_write_i32(&w_buf, &w_len, &w_cap, (int32_t)(s->c))"),
        "Beta_Code (default bit_bound) must serialize as int32\n{beta}"
    );
}

#[test]
fn c_scope_collision_compiles() {
    let c = gen_c();
    if let Err(e) = c_check_compiles(&c) {
        panic!("{e}");
    }
}

fn c_compiler() -> Option<&'static str> {
    ["clang", "gcc"].into_iter().find(|cc| {
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

fn c_check_compiles(c_source: &str) -> Result<(), String> {
    let Some(cc) = c_compiler() else {
        eprintln!("WARNING: skipping C compile-check, no compiler in PATH");
        return Ok(());
    };
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let hdr = dir.path().join("gen.h");
    std::fs::write(&hdr, c_source).map_err(|e| e.to_string())?;
    let c_include = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("zerodds-c-api")
        .join("include");
    let output = Command::new(cc)
        .args(["-std=c99", "-fsyntax-only", "-Wall", "-Wno-unused", "-I"])
        .arg(c_include)
        .arg(&hdr)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "C compile FAILED with {cc}:\n--- header ---\n{c_source}\n--- stderr ---\n{}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}
