// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C++ language binding of DDS-RPC 1.0 §7.11.1, compiled and run.
//!
//! Three things are measured here and none of them can be measured in
//! Rust alone:
//!
//! 1. `runtime/cpp/include/dds/rpc.hpp` compiles under
//!    `-std=c++17 -Wall -Wextra -Werror`, together with the header
//!    `crates/lang-rules/src/emit/interface_cpp.rs` generates for the
//!    same interface. A generated header that named a class the runtime
//!    does not have would be found by a C++ compiler at a user's site
//!    and by nothing before it.
//! 2. `crates/rpc/tests/cpp/rpc_runtime_roundtrip.cpp` runs, and every
//!    class §7.11.1.4.1 and §7.11.1.5 enumerate is used in it — a
//!    request sent through the generated `CalculatorRequester`, answered
//!    through a `Server` over a `Service`, correlated by §7.8.1,
//!    delivered through the listeners, the loaned samples and a
//!    `future`.
//! 3. The two headers of §7.5.1.1.1 take, octet for octet, the layout
//!    `crates/rpc/src/wire_codec.rs` writes. Two implementations of one
//!    clause that agree with themselves and not with each other is the
//!    failure this crate exists to prevent.
//!
//! `runtime/cpp/include/dds/rpc/zerodds.hpp` — the binding of the three
//! abstract entity classes onto the ZeroDDS C API — is syntax-checked in
//! the same run.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_rpc::common_types::{RemoteExceptionCode, ReplyHeader, RequestHeader, SampleIdentity};
use zerodds_rpc::wire_codec::encode_reply_frame;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// The C++ compiler to drive, or `None` where there is none.
///
/// A test that silently passed with no compiler would be a test that
/// measured nothing; a test that failed would refuse to run the suite on
/// a machine with no C++ toolchain. This prints which of the two
/// happened, which is what the Java column's gate does for `javac`.
fn compiler() -> Option<&'static str> {
    ["c++", "clang++", "g++"].into_iter().find(|candidate| {
        Command::new(candidate)
            .arg("--version")
            .output()
            .is_ok_and(|out| out.status.success())
    })
}

/// The generated header for the interface the C++ program includes.
fn generated_header() -> String {
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

    let body = zerodds_lang_rules::generate::source_with_mapping(
        &ast,
        zerodds_lang_cst::Language::Cpp,
        zerodds_lang_rules::emit::interface_rust::rpc_layout_key(),
        zerodds_lang_rules::emit::encode::CodecOptions::default(),
        zerodds_lang_rules::generate::ServiceMapping::Basic,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    // What `zerodds-idlc generate` puts in front of the body: the runtime of
    // the RPC binding, the codec and the conversions of the PSM classes.
    let mut header = String::from("#pragma once\n\n#include <dds/rpc.hpp>\n");
    for include in zerodds_lang_rules::adapter::cpp::includes() {
        header.push_str(&format!("#include <{}>\n", include.path));
    }
    header.push_str("#include <map>\n#include <zerodds/psm/convert.hpp>\n\n");
    header.push_str(&body);
    header
}

/// Where one test's artefacts are built.
///
/// One directory per test, because `cargo test` runs them on threads
/// and two `clang++` invocations writing one binary is a race whose
/// symptom is a truncated executable rather than a message.
fn build_directory(test: &str) -> PathBuf {
    let at = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("dds-rpc-cpp")
        .join(test);
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

/// The translation units of the C++ wire runtime the generated codec uses.
fn wire_runtime_sources(root: &Path) -> Vec<PathBuf> {
    ["writer", "reader", "float128", "md5"]
        .into_iter()
        .map(|unit| root.join(format!("runtime/cpp/src/{unit}.cpp")))
        .collect()
}

/// Compile the round-trip program and hand back the binary.
fn build(compiler: &str, root: &Path, test: &str) -> PathBuf {
    let at = build_directory(test);
    std::fs::write(at.join("calculator.hpp"), generated_header()).expect("the generated header");
    let binary = at.join("rpc_runtime_roundtrip");
    run(
        Command::new(compiler)
            .arg("-std=c++17")
            .args(["-Wall", "-Wextra", "-Werror", "-pthread"])
            .arg(format!("-I{}", at.display()))
            .arg(format!("-I{}", root.join("runtime/cpp/include").display()))
            .arg(format!("-I{}", root.join("crates/cpp/include").display()))
            // The headers of the DDS C++ PSM: with them on the path the
            // exceptions of `dds/rpc.hpp` derive from `dds::core::Exception`
            // (§7.11.1.3), which the program checks.
            .arg(format!("-I{}", root.join("crates/cpp/include").display()))
            .arg(root.join("crates/rpc/tests/cpp/rpc_runtime_roundtrip.cpp"))
            // The generated header has the codec of the topic types, which
            // runs on the wire runtime.
            .args(wire_runtime_sources(root))
            .arg("-o")
            .arg(&binary),
        "compiling the round-trip program",
    );
    binary
}

/// §7.11.1.4.1 and §7.11.1.5: the whole binding, compiled and run.
#[test]
fn the_cpp_rpc_runtime_and_the_generated_header_run_together() {
    let Some(compiler) = compiler() else {
        println!("no C++ compiler on this machine — the C++ binding is not measured here");
        return;
    };
    let root = repository_root();
    let binary = build(compiler, &root, "roundtrip");
    let output = run(&mut Command::new(&binary), "the round-trip program");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("every check held"),
        "the program ran and said: {text}"
    );
}

/// §7.5.1.1.1: the C++ header codec and `wire_codec.rs` write the same
/// octets.
#[test]
fn the_two_headers_take_the_same_octets_in_both_columns() {
    let Some(compiler) = compiler() else {
        println!("no C++ compiler on this machine — the header octets are not compared here");
        return;
    };
    let root = repository_root();
    let binary = build(compiler, &root, "header-octets");
    let output = run(
        Command::new(&binary).arg("--header-octets"),
        "the header-octet dump",
    );
    let dumped: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(dumped.len(), 2, "one line per header: {dumped:?}");

    let identity = SampleIdentity::new([0x42; 16], 0x0102_0304_0506_0708);
    // The header alone: the padding that aligns the `Call` union belongs to the
    // sample, not to the header (§7.5.1.1.6 rule 2).
    let request = RequestHeader::new(identity, "calc-A")
        .to_cdr_le()
        .expect("encode request");
    let reply = encode_reply_frame(
        &ReplyHeader::new(identity, RemoteExceptionCode::InvalidArgument),
        &[],
    );

    // SequenceNumber is two declared 32-bit fields, high then low, as in the
    // vendor IDL/oracle. This independently rejects agreement on a swapped u64.
    let sequence_octets = [4, 3, 2, 1, 8, 7, 6, 5];
    assert_eq!(&request[16..24], &sequence_octets);
    assert_eq!(&reply[16..24], &sequence_octets);

    assert_eq!(dumped[0], hex(&request), "the RequestHeader octets differ");
    assert_eq!(dumped[1], hex(&reply), "the ReplyHeader octets differ");
}

/// The ZeroDDS binding of the three abstract entity classes compiles
/// against the public C header.
#[test]
fn the_zerodds_entity_binding_compiles() {
    let Some(compiler) = compiler() else {
        println!("no C++ compiler on this machine — the ZeroDDS binding is not compiled here");
        return;
    };
    let root = repository_root();
    run(
        Command::new(compiler)
            .arg("-std=c++17")
            .args(["-Wall", "-Wextra", "-Werror", "-fsyntax-only"])
            .arg(format!("-I{}", root.join("runtime/cpp/include").display()))
            .arg(format!("-I{}", root.join("crates/cpp/include").display()))
            .arg(format!(
                "-I{}",
                root.join("crates/zerodds-c-api/include").display()
            ))
            .args(["-x", "c++"])
            .arg(root.join("runtime/cpp/include/dds/rpc/zerodds.hpp")),
        "syntax-checking the ZeroDDS entity binding",
    );
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|octet| format!("{octet:02x}"))
        .collect::<String>()
}
