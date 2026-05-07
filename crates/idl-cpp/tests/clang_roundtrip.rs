//! Optionaler Roundtrip-Test: IDL → C++-Header → `clang -fsyntax-only`.
//!
//! Default ignored, weil clang nicht in jeder CI/Dev-Umgebung verfuegbar
//! ist. Run via `cargo test -p zerodds-idl-cpp -- --ignored`.
//!
//! Wenn clang im PATH ist und der Test grun ist, beweist das die
//! Syntax-Korrektheit (nicht Semantik-Korrektheit) des generierten
//! C++17-Headers.

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

use std::io::Write;
use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CppGenOptions, generate_cpp_header};

fn clang_available() -> bool {
    Command::new("clang")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run_clang_syntax_only(cpp: &str) -> Result<(), String> {
    let mut tmp = tempfile::Builder::new()
        .suffix(".hpp")
        .tempfile()
        .map_err(|e| format!("tempfile: {e}"))?;
    tmp.write_all(cpp.as_bytes())
        .map_err(|e| format!("write: {e}"))?;
    tmp.flush().map_err(|e| format!("flush: {e}"))?;
    let path = tmp.path();
    let output = Command::new("clang")
        .args(["-std=c++17", "-fsyntax-only", "-x", "c++"])
        .arg(path)
        .output()
        .map_err(|e| format!("spawn: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "clang failed:\nstderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

#[test]
#[ignore = "requires clang in PATH"]
fn prim_struct_passes_clang_syntax_check() {
    if !clang_available() {
        println!("clang not available, skipping");
        return;
    }
    let src = include_str!("fixtures/prim_struct.idl");
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    let cpp = generate_cpp_header(&ast, &CppGenOptions::default()).expect("gen");
    if let Err(e) = run_clang_syntax_only(&cpp) {
        panic!("clang syntax-check failed:\n{e}");
    }
}
