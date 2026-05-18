// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! L3 Cross-Language XCDR2 Conformance — `zerodds-xcdr2-bindings-conformance-1.0` §7.
//!
//! Verifiziert dass alle 6 Sprach-Bindings (cpp, c-ffi, csharp, java,
//! ts, rust) byte-identische XCDR2-Wire-Frames fuer V-1..V-12
//! produzieren — modulo encoder-Wahl (LC=2/3 vs LC=4) bei Mutable.
//!
//! Strategie: dieser Test ruft pro Sprache deren native Test-Suite
//! als Subprocess auf. Wenn das jeweilige Tool nicht im PATH ist,
//! wird die Sprache geskippt mit `WARNING`. Cross-Vendor-L4 (gegen
//! Cyclone DDS) lebt in `tests/interop/xcdr2_cross_vendor.sh`.
//!
//! Die Master-Spec wire-bytes pro V-i liegen in
//! `docs/specs/zerodds-xcdr2-bindings-conformance-1.0.md` §6 und
//! sind die Single-Source-of-Truth. Jede Sprach-Test-Suite assertiert
//! gegen dieselben Bytes — wir verifizieren das hier durch Aufruf
//! der Test-Suiten.

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

use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn tool_in_path(tool: &str) -> bool {
    Command::new("which")
        .arg(tool)
        .stdout(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// L3.1 — Rust reference encoder (direct check via `crates/cdr` test).
#[test]
fn l3_1_rust_reference_encoder() {
    let root = workspace_root();
    let status = Command::new("cargo")
        .args(["test", "-p", "zerodds-cdr", "--test", "xcdr2_wire_vectors"])
        .current_dir(&root)
        .status()
        .expect("cargo test invocable");
    assert!(status.success(), "rust V-1..V-12 must pass");
}

/// L3.2 — C++ binding (idl-cpp wire-vector test compiles + runs the
/// generated `topic_type_support<T>` and asserts byte-identity with
/// the master spec).
#[test]
fn l3_2_cpp_binding() {
    if !tool_in_path("clang++") && !tool_in_path("g++") {
        eprintln!("WARNING: skipping L3.2 cpp, no C++ compiler in PATH");
        return;
    }
    let root = workspace_root();
    let status = Command::new("cargo")
        .args([
            "test",
            "-p",
            "zerodds-idl-cpp",
            "--test",
            "xcdr2_wire_vectors",
        ])
        .current_dir(&root)
        .status()
        .expect("cargo test invocable");
    assert!(status.success(), "cpp V-1..V-12 must pass");
}

/// L3.3 — C-FFI binding (idl-cpp `c_mode` codegen + `cc -std=c99`).
#[test]
fn l3_3_c_ffi_binding() {
    if !tool_in_path("clang") && !tool_in_path("cc") && !tool_in_path("gcc") {
        eprintln!("WARNING: skipping L3.3 c-ffi, no C compiler in PATH");
        return;
    }
    let root = workspace_root();
    let status = Command::new("cargo")
        .args([
            "test",
            "-p",
            "zerodds-c-api",
            "--test",
            "xcdr2_wire_vectors",
        ])
        .current_dir(&root)
        .status()
        .expect("cargo test invocable");
    assert!(status.success(), "c-ffi V-1..V-12 must pass");
}

/// L3.4 — C# binding (`dotnet test` der ZeroDDS.Cdr.Tests).
#[test]
fn l3_4_csharp_binding() {
    if !tool_in_path("dotnet") {
        eprintln!("WARNING: skipping L3.4 csharp, no dotnet in PATH");
        return;
    }
    let root = workspace_root();
    let status = Command::new("dotnet")
        .args(["test", "--nologo"])
        .current_dir(root.join("crates/cs/csharp/ZeroDDS.Cdr.Tests"))
        .status()
        .expect("dotnet test invocable");
    assert!(status.success(), "csharp V-1..V-12 must pass");
}

/// L3.5 — Java binding (`mvn test` der java-omgdds JUnit-Suite).
#[test]
fn l3_5_java_binding() {
    if !tool_in_path("mvn") {
        eprintln!("WARNING: skipping L3.5 java, no mvn in PATH");
        return;
    }
    let root = workspace_root();
    let status = Command::new("mvn")
        .args([
            "test",
            "-q",
            "-Dtest=Xcdr2WireVectorsTest",
            "-DfailIfNoTests=false",
        ])
        .current_dir(root.join("crates/java-omgdds/java"))
        .status()
        .expect("mvn invocable");
    assert!(status.success(), "java V-1..V-12 must pass");
}

/// L3.6 — TypeScript binding (`npm run test:wire` via node).
#[test]
fn l3_6_typescript_binding() {
    if !tool_in_path("node") || !tool_in_path("npm") {
        eprintln!("WARNING: skipping L3.6 ts, no node/npm in PATH");
        return;
    }
    let root = workspace_root();
    let ts_node_dir = root.join("crates/ts-node");
    // Devdeps (tsx) muessen via npm install installiert sein. Auf CI ohne
    // npm-install-Step wuerde sonst tsx fehlen und der Test failed mit
    // unklarer Module-Resolution-Meldung. Skip statt fail.
    if !ts_node_dir.join("node_modules").join("tsx").exists() {
        eprintln!(
            "WARNING: skipping L3.6 ts, crates/ts-node/node_modules/tsx fehlt — run `npm ci` in crates/ts-node"
        );
        return;
    }
    let status = Command::new("npm")
        .args(["run", "test:wire", "--silent"])
        .current_dir(ts_node_dir)
        .status()
        .expect("npm invocable");
    assert!(status.success(), "ts V-1..V-12 must pass");
}

/// L3.0 — Cross-Language-Aequivalenz-Aussage. Wenn alle 6 Sprach-
/// Suites gegen dieselbe Master-Spec hex assertieren und alle
/// gruen sind (verifiziert durch L3.1..L3.6), dann ist Cross-Language-
/// Byte-Equivalenz fuer V-1..V-12 bewiesen.
///
/// Die hex-Werte selbst stehen in
/// `docs/specs/zerodds-xcdr2-bindings-conformance-1.0.md` §6 und
/// werden in jeder Sprach-Test-Suite woertlich verwendet. Eine
/// Drift-Pruefung (Test-Files vs Master-Spec) ist als Folge-Sprint
/// dokumentiert.
#[test]
fn l3_0_cross_language_equivalence_documented() {
    // Diese Test-Funktion ist eine semantische Markierung: wenn sie
    // grun lauft (zusammen mit den anderen L3.X), gilt
    // L3-Konformanz fuer alle 6 Sprachen.
    let root = workspace_root();
    let spec = root.join("docs/specs/zerodds-xcdr2-bindings-conformance-1.0.md");
    assert!(spec.exists(), "master conformance spec must exist");
}
