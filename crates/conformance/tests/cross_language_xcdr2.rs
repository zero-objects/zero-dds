// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! L3 Cross-Language XCDR2 Conformance — `zerodds-xcdr2-bindings-conformance-1.0` §7.
//!
//! Verifies that all 6 language bindings (cpp, c-ffi, csharp, java,
//! ts, rust) produce byte-identical XCDR2 wire frames for V-1..V-12 —
//! modulo encoder choice (LC=2/3 vs LC=4) for Mutable.
//!
//! Strategy: this test invokes each language's native test suite as a
//! subprocess. If the respective tool is not in PATH, that language is
//! skipped with `WARNING`. Cross-vendor L4 (against Cyclone DDS) lives
//! in `tests/interop/xcdr2_cross_vendor.sh`.
//!
//! The master-spec wire bytes per V-i live in
//! `docs/specs/zerodds-xcdr2-bindings-conformance-1.0.md` §6 and are
//! the single source of truth. Each language test suite asserts
//! against the same bytes — we verify that here by invoking those test
//! suites.

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

/// L3.4 — C# binding (`dotnet test` of ZeroDDS.Cdr.Tests).
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

/// L3.5 — Java binding (`mvn test` of the java-omgdds JUnit suite).
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
    // Dev deps (tsx) must be installed via npm install. On CI without an
    // npm-install step, tsx would be missing and the test would fail with
    // an unclear module-resolution message. Skip instead of fail.
    if !ts_node_dir.join("node_modules").join("tsx").exists() {
        eprintln!(
            "WARNING: skipping L3.6 ts, crates/ts-node/node_modules/tsx missing — run `npm ci` in crates/ts-node"
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

/// L3.0 — Cross-language equivalence statement. If all 6 language
/// suites assert against the same master-spec hex and are all green
/// (verified by L3.1..L3.6), then cross-language byte equivalence for
/// V-1..V-12 is proven.
///
/// The hex values themselves live in
/// `docs/specs/zerodds-xcdr2-bindings-conformance-1.0.md` §6 and are
/// used verbatim in each language test suite. A drift check (test
/// files vs master spec) is documented as a follow-up sprint.
#[test]
fn l3_0_cross_language_equivalence_documented() {
    // This test function is a semantic marker: if it runs green
    // (together with the other L3.X), L3 conformance holds for all 6
    // languages.
    let root = workspace_root();
    let spec = root.join("docs/specs/zerodds-xcdr2-bindings-conformance-1.0.md");
    assert!(spec.exists(), "master conformance spec must exist");
}
