// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! Cargo-triggered C++ smoke test: compiles
//! `crates/cpp/tests/smoke_dds_psm.cpp` against libzerodds.dylib and
//! checks that the DDS-PSM-Cxx 1.0 header interface works end-to-end.
//!
//! Requires `clang++` (or `c++`) in PATH.

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
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(PathBuf::from)
        .expect("workspace root")
}

#[test]
fn dds_psm_cxx_smoke() {
    let root = workspace_root();
    let cpp_inc = root.join("crates/cpp/include");
    let c_inc = root.join("crates/zerodds-c-api/include");
    let src = root.join("crates/cpp/tests/smoke_dds_psm.cpp");

    // Build cdylib first.
    let status = Command::new(env!("CARGO"))
        .args(["build", "-p", "zerodds-c-api", "--release"])
        .current_dir(&root)
        .status()
        .expect("cargo build");
    assert!(status.success(), "cargo build zerodds-c-api failed");

    // Locate libzerodds.dylib (macOS) / libzerodds.so (Linux).
    // CI sets --target=<triple>, so the output lib is under
    // target/<triple>/release/; locally without a target under
    // target/release/. Search both variants + fall back to deps/.
    let lib_names = ["libzerodds.dylib", "libzerodds.so"];
    let candidate_dirs = {
        let mut v: Vec<PathBuf> = Vec::new();
        v.push(root.join("target/release"));
        v.push(root.join("target/release/deps"));
        if let Ok(entries) = std::fs::read_dir(root.join("target")) {
            for e in entries.flatten() {
                let rel = e.path().join("release");
                if rel.is_dir() {
                    v.push(rel.clone());
                    v.push(rel.join("deps"));
                }
            }
        }
        v
    };
    let lib_dir = candidate_dirs
        .into_iter()
        .find(|d| lib_names.iter().any(|n| d.join(n).exists()))
        .expect("libzerodds.{dylib,so} not found in any target/*/release[/deps]");

    // Compile.
    let bin = std::env::temp_dir().join(format!("zerodds_cpp_smoke_{}", std::process::id()));
    let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".to_string());
    let out = Command::new(&cxx)
        .args(["-std=c++17", "-Wall"])
        .arg("-I")
        .arg(&cpp_inc)
        .arg("-I")
        .arg(&c_inc)
        .arg(&src)
        .arg("-L")
        .arg(&lib_dir)
        .args(["-lzerodds"])
        .arg("-o")
        .arg(&bin)
        .output()
        .expect("c++ compile");
    if !out.status.success() {
        panic!(
            "c++ compile failed:\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    // Run.
    let out = Command::new(&bin)
        .env(
            if cfg!(target_os = "macos") {
                "DYLD_LIBRARY_PATH"
            } else {
                "LD_LIBRARY_PATH"
            },
            &lib_dir,
        )
        .output()
        .expect("run smoke");
    let _ = std::fs::remove_file(&bin);
    if !out.status.success() {
        panic!(
            "smoke test failed:\nbin={}\nlib_dir={}\nstatus={:?}\nstdout={:?}\nstderr={:?}",
            bin.display(),
            lib_dir.display(),
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("OK"), "expected OK marker, got: {}", s);
}
