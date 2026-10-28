// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Rust binding's `Calculator.add` request/reply octets, measured
//! against the C++ column's — round-2 feedback's wire test: "the Rust
//! request/reply bytes for the Calculator calls equal what the C++
//! binding's types encode".
//!
//! `crates/rpc/tests/cpp_rpc_runtime.rs` already holds the two columns'
//! `RequestHeader`/`ReplyHeader` octets against each other; this is the
//! same comparison one layer further in, over the operation-specific
//! `Calculator_Call`/`Calculator_Return` payload §7.5.1.1.4/§7.5.1.1.5
//! define, which that file's Calculator has no operation exercising.
//! The compiler-discovery/build/run pattern below is
//! `cpp_rpc_runtime.rs`'s own — copied rather than shared across the
//! crate boundary (`zerodds-rpc-rust-e2e` cannot depend on
//! `zerodds-rpc`'s test-only code, and `zerodds-rpc` cannot depend on
//! `zerodds-rpc-rust-e2e` without a cycle), but every generation call it
//! makes (`zerodds_lang_rules::emit::interface_cpp`,
//! `zerodds_lang_render::cpp::decl`) is the same public library code
//! that file calls, not a reimplementation.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::path::PathBuf;
use std::process::Command;

use zerodds_rpc_rust_e2e::calculator::demo::{
    Calculator_Call, Calculator_Return, Calculator_add_In, Calculator_add_Out,
    Calculator_add_Result,
};
use zerodds_wire::{Endianness, Writer};

/// The C++ compiler to drive, or `None` where there is none —
/// `crates/rpc/tests/cpp_rpc_runtime.rs::compiler`.
fn compiler() -> Option<&'static str> {
    ["c++", "clang++", "g++"].into_iter().find(|candidate| {
        Command::new(candidate)
            .arg("--version")
            .output()
            .is_ok_and(|out| out.status.success())
    })
}

/// The generated C++ header for the same `Calculator` this crate's own
/// `build.rs` generates in Rust, trimmed to the one operation this test
/// measures — `crates/rpc/tests/cpp_rpc_runtime.rs::generated_header`.
fn generated_cpp_header() -> String {
    let source = "
        module robot {
          interface Calculator {
            long add(in long a, in long b);
          };
        };
    ";
    let parsed = zerodds_idl_parse::parse(source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "calculator.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    // The header reads the fields of the codec's structures: the native mapping.
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );

    let body = zerodds_lang_rules::emit::interface_cpp::cpp_interface_items(&ast)
        .unwrap_or_else(|error| panic!("{error}"))
        .iter()
        .map(zerodds_lang_render::cpp::decl)
        .collect::<Vec<_>>()
        .join("\n");

    let mut header = String::from("#pragma once\n\n#include <dds/rpc.hpp>\n");
    for include in zerodds_lang_rules::adapter::cpp::includes() {
        header.push_str(&format!("#include <{}>\n", include.path));
    }
    header.push('\n');
    header.push_str(&body);
    header.push('\n');
    header
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn build_directory() -> PathBuf {
    let at = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("wire-bytes-match-cpp");
    std::fs::create_dir_all(&at).expect("the build directory");
    at
}

fn run(command: &mut Command, what: &str) -> std::process::Output {
    let output = command.output().unwrap_or_else(|e| panic!("{what}: {e}"));
    assert!(
        output.status.success(),
        "{what} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// Compiles `calculator_add_wire.cpp` against the generated header and
/// hands back its two hex lines: request payload, reply payload.
fn cpp_octets(compiler: &str) -> [String; 2] {
    let root = repository_root();
    let at = build_directory();
    std::fs::write(at.join("calculator.hpp"), generated_cpp_header())
        .expect("the generated header");
    let binary = at.join("calculator_add_wire");
    run(
        Command::new(compiler)
            .arg("-std=c++17")
            .args(["-Wall", "-Wextra", "-Werror"])
            .arg(format!("-I{}", at.display()))
            .arg(format!("-I{}", root.join("runtime/cpp/include").display()))
            .arg(root.join("crates/rpc-rust-e2e/tests/cpp/calculator_add_wire.cpp"))
            // The generated header's codec runs on the wire runtime.
            .args(
                ["writer", "reader", "float128", "md5"]
                    .map(|unit| root.join(format!("runtime/cpp/src/{unit}.cpp"))),
            )
            .arg("-o")
            .arg(&binary),
        "compiling calculator_add_wire.cpp",
    );
    let output = run(&mut Command::new(&binary), "calculator_add_wire");
    let lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(lines.len(), 2, "one line per payload: {lines:?}");
    [lines[0].clone(), lines[1].clone()]
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The Rust column's octets for the same call, through the actual
/// generated `Calculator_Call`/`Calculator_Return` types
/// `crates/rpc-rust-e2e/build.rs` compiles in — not a reimplementation
/// of the layout, the generator's own output.
fn rust_octets() -> [String; 2] {
    let mut request = Writer::new(Endianness::Little);
    Calculator_Call::Add(Calculator_add_In { a: 3, b: 4 })
        .encode(&mut request)
        .expect("encode the request");

    let mut reply = Writer::new(Endianness::Little);
    Calculator_Return::Add(Calculator_add_Result::Ok(Calculator_add_Out { return_: 7 }))
        .encode(&mut reply)
        .expect("encode the reply");

    [hex(&request.finish()), hex(&reply.finish())]
}

#[test]
fn the_rust_and_cpp_columns_encode_calculator_add_identically() {
    let Some(compiler) = compiler() else {
        println!("no C++ compiler on this machine — the wire comparison is not measured here");
        return;
    };
    let cpp = cpp_octets(compiler);
    let rust = rust_octets();
    assert_eq!(rust[0], cpp[0], "the request payload octets differ");
    assert_eq!(rust[1], cpp[1], "the reply payload octets differ");
}
