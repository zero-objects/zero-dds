// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The function-call style of DDS-RPC 1.0 §7.11.1.5 in C++, generated,
//! compiled and run.
//!
//! `crates/lang-rules/src/emit/interface_cpp.rs` writes, for an interface,
//! the topic types, their codec, a `${interface}Client` whose operations
//! call through a `dds::rpc::Requester`, and a `${interface}Service` that a
//! `dds::rpc::Server` dispatches. `tests/cpp/function_call_roundtrip.cpp`
//! uses them over an in-memory binding: synchronous and asynchronous calls
//! with every parameter direction, a declared exception, a remote
//! exception code, attributes, the octets of the two CDR forms, run-time
//! QoS and deactivation (§7.9.3).
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::path::{Path, PathBuf};
use std::process::Command;

const INTERFACE: &str = "
    module robot {
      exception TooFast { long limit; string why; };
      exception template { long code; };
      struct Pose { double x; double y; };
      interface Calculator {
        long add(in long a, in long b);
        void reset();
        long div(in long a, in long b, out long rest) raises (TooFast);
        Pose move(in Pose start, inout long steps);
        string label(in string prefix);
        double scale(in double factor, in long long big);
        long risky(in long x) raises (template);
        attribute long total;
        readonly attribute Pose origin;
      };
    };
";

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn compiler() -> Option<&'static str> {
    ["c++", "clang++", "g++"].into_iter().find(|candidate| {
        Command::new(candidate)
            .arg("--version")
            .output()
            .is_ok_and(|out| out.status.success())
    })
}

/// The header the generator writes for [`INTERFACE`]: the data types, then
/// the binding.
fn generated_header() -> String {
    let parsed = zerodds_idl_parse::parse(INTERFACE);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "calculator.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    // The whole generator path: the data types, the PSM classes of the
    // default mapping (DDS-PSM-Cxx 1.0, Table 7.1) and the RPC binding.
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

fn build(compiler: &str, root: &Path) -> PathBuf {
    let at = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("dds-rpc-cpp")
        .join("function-call");
    std::fs::create_dir_all(&at).expect("the build directory");
    std::fs::write(at.join("calculator.hpp"), generated_header()).expect("the generated header");
    let binary = at.join("function_call_roundtrip");
    let mut command = Command::new(compiler);
    command
        .arg("-std=c++17")
        .args(["-Wall", "-Wextra", "-Werror", "-pthread"])
        .arg(format!("-I{}", at.display()))
        .arg(format!("-I{}", root.join("runtime/cpp/include").display()))
        .arg(format!("-I{}", root.join("crates/cpp/include").display()))
        .arg(root.join("crates/rpc/tests/cpp/function_call_roundtrip.cpp"));
    for unit in ["writer", "reader", "float128", "md5"] {
        command.arg(root.join(format!("runtime/cpp/src/{unit}.cpp")));
    }
    run(
        command.arg("-o").arg(&binary),
        "compiling the function-call program",
    );
    binary
}

/// §7.11.1.5: the generated client and service, a server, and the codec
/// between them.
#[test]
fn the_generated_client_calls_the_generated_service() {
    let Some(compiler) = compiler() else {
        println!(
            "no C++ compiler on this machine — the function-call binding is not measured here"
        );
        return;
    };
    let root = repository_root();
    let binary = build(compiler, &root);
    let output = run(&mut Command::new(&binary), "the function-call program");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("every check held"),
        "the program ran and said: {text}"
    );
}
