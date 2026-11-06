// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C++ binding `runtime/cpp/include/dds/rpc/zerodds.hpp` against the
//! real C library.
//!
//! The other C++ tests of this crate run the binding over an in-memory bus or
//! over C symbols the probe defines itself. These compile
//! `tests/cpp/real_library_roundtrip.cpp` and
//! `tests/cpp/real_library_evolution.cpp` against `libzerodds`
//! (`zerodds-c-api`, built by `tests/integration/rpc_cpp.sh` into a target
//! directory of its own) and runs it on a leased domain: the
//! generated Calculator client and service of the Enhanced Service Mapping
//! on two runtimes of one domain, with the discovery extension of §7.6.2
//! (the client waits for both endpoints of the service, a named instance is
//! addressed by its name), the related sample identity of §7.8.2 and the
//! samples crossing the RTPS stack. The Basic mapping is not run here: it
//! relies on the plain DDS match, and §7.6.1 says its replies can be lost
//! to a discovery race.
//!
//! Run on Linux, or where `ZD_RUN_GENERATED=1` asks for it: the program
//! starts threads and opens sockets, which is for the test host. The build
//! of the library and the program does not fit the 25 s of a crate-tier
//! test, so the test runs in the job `integration-sdk`.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stdout)]

use std::path::PathBuf;
use std::process::Command;

const CALCULATOR: &str = "
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

const RETURN_TYPES: &str = "
    // The return type of an operation (DDS-RPC 1.0 §7.7.2.5.5): a mutable
    // structure that gained a member (assignable) and a type that widened
    // from long to long long (not assignable).
    module ev_ret_old {
      @mutable struct Pair { long a; };
      interface Svc { Pair get(); };
    };
    module ev_ret_new {
      @mutable struct Pair { long a; long b; };
      interface Svc { Pair get(); };
    };
    module ev_wide_old {
      interface Svc { long value(); };
    };
    module ev_wide_new {
      interface Svc { long long value(); };
    };
";

const EVOLUTION: &str = "
    // Two versions of one interface and a third whose parameter changed its
    // type (DDS-RPC 1.0 §7.7.2.5): the topics carry the TypeObjects of their
    // types, so the first pair matches and the second does not.
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
    module ev_changed {
      exception E1 { long code; };
      interface Svc {
        long add(in string a, in long b);
        long legacy();
        @Mutable void mix(in long x, in long y);
        long fail() raises (E1);
      };
    };
";

mod common;
use common::root;

fn generated_header(idl: &str) -> String {
    let parsed = zerodds_idl_parse::parse(idl);
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

/// Compiles `tests/cpp/<program>.cpp` with the header generated from `idl`
/// (as `<header>`) against `libzerodds` and runs it on a leased domain; the
/// program prints "every check held".
fn compile_and_run(program: &str, header: &str, idl: &str) {
    let root = root();
    let lib_dir = common::library_dir();
    let lease = zerodds_test_domains::lease_domain();
    let at = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("dds-rpc-cpp")
        .join(program);
    std::fs::create_dir_all(&at).expect("the build directory");
    std::fs::write(at.join(header), generated_header(idl)).expect("the generated header");
    let binary = at.join(program);
    let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".to_string());
    let mut compile = Command::new(cxx);
    compile.args(["-std=c++17", "-Wall", "-Wextra", "-pthread"]);
    run(
        compile
            .arg(format!("-I{}", at.display()))
            .arg(format!("-I{}", root.join("runtime/cpp/include").display()))
            .arg(format!("-I{}", root.join("crates/cpp/include").display()))
            .arg(format!(
                "-I{}",
                root.join("crates/zerodds-c-api/include").display()
            ))
            .arg(root.join(format!("crates/rpc/tests/cpp/{program}.cpp")))
            .args(
                ["writer", "reader", "float128", "md5"]
                    .map(|unit| root.join(format!("runtime/cpp/src/{unit}.cpp"))),
            )
            .arg(format!("-L{}", lib_dir.display()))
            .arg("-lzerodds")
            .arg("-o")
            .arg(&binary),
        "compiling the real-library program",
    );
    let output = run(
        Command::new(&binary)
            .arg(lease.id().to_string())
            .env("LD_LIBRARY_PATH", lib_dir)
            .env("DYLD_LIBRARY_PATH", lib_dir),
        "the real-library program",
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("every check held"),
        "the program said: {text}"
    );
}

fn on_the_test_host() -> bool {
    if cfg!(target_os = "linux") || std::env::var("ZD_RUN_GENERATED").is_ok_and(|v| v == "1") {
        return true;
    }
    println!("not Linux and ZD_RUN_GENERATED is not 1: the program is compiled by no one here");
    false
}

/// The Calculator interface: operations, exceptions, attributes, named
/// instances (§7.6.2), the related sample identity (§7.8.2).
#[test]
#[ignore = "integration-sdk: builds a C++ program against libzerodds, which tests/integration/rpc_cpp.sh builds first; run with --run-ignored"]
fn the_cpp_binding_runs_against_libzerodds() {
    if on_the_test_host() {
        compile_and_run("real_library_roundtrip", "calculator.hpp", CALCULATOR);
    }
}

/// §7.7.2.5.1–§7.7.2.5.3: an old client and a new service match where the
/// types are assignable (an added operation, a reordered `@Mutable`
/// operation) and do not where they are not (a parameter of another type).
/// In a program of its own: with the Calculator's, the build (13.1 s) and
/// the run (7.7 s) came to 22.0 s of the 25 s of a test on the Linux test
/// host.
#[test]
#[ignore = "integration-sdk: builds a C++ program against libzerodds, which tests/integration/rpc_cpp.sh builds first; run with --run-ignored"]
fn rpc_7_7_2_5_the_cpp_binding_matches_by_assignability_against_libzerodds() {
    if on_the_test_host() {
        compile_and_run("real_library_evolution", "evolution.hpp", EVOLUTION);
    }
}

/// §7.7.2.5.5: a return type that gained a member is assignable, so the old
/// client reads the new service's reply; one widened from `long` to
/// `long long` is not, and the endpoints do not match.
#[test]
#[ignore = "integration-sdk: builds a C++ program against libzerodds, which tests/integration/rpc_cpp.sh builds first; run with --run-ignored"]
fn rpc_7_7_2_5_5_a_changed_return_type_matches_by_assignability_against_libzerodds() {
    if on_the_test_host() {
        compile_and_run(
            "real_library_return_types",
            "return_types.hpp",
            RETURN_TYPES,
        );
    }
}
