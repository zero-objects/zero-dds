// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The function-call style of DDS-RPC 1.0 §7.11.1.5 over the Enhanced
//! Service Mapping (§7.5.1.2) in C++, generated, compiled and run.
//!
//! `crates/lang-rules/src/emit/interface_cpp/enhanced.rs` writes, for an
//! interface under `ServiceMapping::Enhanced`, the `@Choice` topic types,
//! their PL_CDR2 codec (`enhanced_codec.rs`), a `${interface}Client` whose
//! operations set the member of the operation in the request and read the
//! `Result` of that member from the reply, and a `${interface}Service` that
//! dispatches on the member that is present.
//! `tests/cpp/enhanced_function_call_roundtrip.cpp` uses them over an
//! in-memory binding and compares the octets with those of the clause.
//!
//! The program is compiled everywhere there is a C++ compiler and run on
//! Linux, or where `ZD_RUN_GENERATED=1` asks for it: a run starts threads
//! and is for the test host, not for a developer machine.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_lang_rules::adapter::interface::member_id;

const INTERFACE: &str = "
    module robot {
      exception TooFast { long limit; string why; };
      interface Base2 {
        long ident();
      };
      interface Calc : Base2 {
        long add(in long a, in long b);
        void reset();
        long div(in long a, in long b, out long rest) raises (TooFast);
        string label(in string prefix);
        void note(in string text);
        attribute long total;
      };
      interface Fast {
        @Mutable long square(in long x);
      };
    };
    // Two versions of one interface (enhanced_evolution_roundtrip.cpp):
    // operations added and removed, the parameters of a @Mutable operation
    // reordered, an exception added.
    module ev_old {
      exception E1 { long code; };
      interface Svc {
        long add(in long a, in long b);
        long legacy();
        @Mutable void mix(in long x, in long y);
        long fail() raises (E1);
      };
    };
    module ev_new {
      exception E1 { long code; };
      exception E2 { long code; };
      interface Svc {
        long add(in long a, in long b);
        long extra();
        @Mutable void mix(in long y, in long x);
        long fail() raises (E1, E2);
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
/// the Enhanced binding.
fn generated_header() -> String {
    let parsed = zerodds_idl_parse::parse(INTERFACE);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "calc.idl");
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
        zerodds_lang_rules::generate::ServiceMapping::Enhanced,
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
    build_program(compiler, root, "enhanced_function_call_roundtrip")
}

/// Compiles `tests/cpp/<program>.cpp` against the generated header; one
/// directory per program, because `cargo test` runs the tests on threads.
fn build_program(compiler: &str, root: &Path, program: &str) -> PathBuf {
    let at = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("dds-rpc-cpp")
        .join(program);
    std::fs::create_dir_all(&at).expect("the build directory");
    std::fs::write(at.join("calc.hpp"), generated_header()).expect("the generated header");
    let binary = at.join(program);
    let mut command = Command::new(compiler);
    command
        .arg("-std=c++17")
        .args(["-Wall", "-Wextra", "-Werror", "-pthread"])
        // §7.5.1.2.1.2: the member id is the HASH of the member name, 28
        // bits of it.
        .arg(format!("-DZD_ID_ADD={}u", member_id("add")))
        .arg(format!("-DZD_ID_RESULT={}u", member_id("result")))
        .arg(format!("-DZD_ID_REMOTE_EX={}u", member_id("remoteEx")))
        .arg(format!("-DZD_ID_X={}u", member_id("x")))
        .arg(format!("-I{}", at.display()))
        .arg(format!("-I{}", root.join("runtime/cpp/include").display()))
        .arg(format!("-I{}", root.join("crates/cpp/include").display()))
        .arg(root.join(format!("crates/rpc/tests/cpp/{program}.cpp")));
    for unit in ["writer", "reader", "float128", "md5"] {
        command.arg(root.join(format!("runtime/cpp/src/{unit}.cpp")));
    }
    run(
        command.arg("-o").arg(&binary),
        "compiling the Enhanced function-call program",
    );
    binary
}

/// §7.5.1.2, §7.11.1.5: the generated client and service over the Enhanced
/// topic types, and the codec between them.
#[test]
fn the_generated_enhanced_client_calls_the_generated_enhanced_service() {
    let Some(compiler) = compiler() else {
        println!("no C++ compiler on this machine — the Enhanced binding is not measured here");
        return;
    };
    let root = repository_root();
    let binary = build(compiler, &root);
    if !(cfg!(target_os = "linux") || std::env::var("ZD_RUN_GENERATED").is_ok_and(|v| v == "1")) {
        println!(
            "compiled, not run: this host does not run generated programs (set ZD_RUN_GENERATED=1)"
        );
        return;
    }
    let output = run(
        &mut Command::new(&binary),
        "the Enhanced function-call program",
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("every check held"),
        "the program ran and said: {text}"
    );
}

/// §7.7.2: two generated versions of one interface run against each other
/// over shared topic names: an operation one version lacks is answered
/// `REMOTE_EX_UNKNOWN_OPERATION`, the parameters of a `@Mutable` operation
/// follow their names when they are reordered, and an exception the client
/// does not know is `REMOTE_EX_UNKNOWN_EXCEPTION`.
#[test]
fn two_generated_cpp_versions_of_an_interface_run_against_each_other() {
    let Some(compiler) = compiler() else {
        println!("no C++ compiler on this machine — the evolution is not measured here");
        return;
    };
    let root = repository_root();
    let binary = build_program(compiler, &root, "enhanced_evolution_roundtrip");
    if !(cfg!(target_os = "linux") || std::env::var("ZD_RUN_GENERATED").is_ok_and(|v| v == "1")) {
        println!(
            "compiled, not run: this host does not run generated programs (set ZD_RUN_GENERATED=1)"
        );
        return;
    }
    let output = run(&mut Command::new(&binary), "the evolution program");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("every check held"),
        "the program ran and said: {text}"
    );
}
