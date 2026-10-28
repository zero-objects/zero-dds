// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A C++ test program built against the C ABI library and run: the C ABI
//! library is built into a target directory of its own, the program is
//! compiled against the headers, run with a leased domain as its argument,
//! and has to exit 0 and print its OK line.
#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(PathBuf::from)
        .expect("workspace root")
}

/// Builds the C ABI library, compiles `source` (relative to the workspace
/// root) into a binary named after `bin_stem`, runs it and asserts that it
/// succeeded and printed `ok_line`.
pub fn build_compile_run(source: &str, bin_stem: &str, ok_line: &str) {
    let root = workspace_root();
    // Its own target directory, as the smoke test does and for its reason: a `cargo build`
    // inside a `cargo test` must not write where the outer run reads.
    let build_dir = root.join("target/cpp-smoke");
    let built = Command::new(env!("CARGO"))
        .args(["build", "-p", "zerodds-c-api", "--release"])
        .arg("--target-dir")
        .arg(&build_dir)
        .current_dir(&root)
        .output()
        .expect("cargo build");
    assert!(
        built.status.success(),
        "cargo build zerodds-c-api failed:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let release = build_dir.join("release");
    let bin = std::env::temp_dir().join(format!("{bin_stem}_{}", std::process::id()));
    let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".to_string());
    let compiled = Command::new(&cxx)
        .args(["-std=c++17", "-Wall", "-Werror", "-I"])
        .arg(root.join("crates/cpp/include"))
        .arg("-I")
        .arg(root.join("crates/zerodds-c-api/include"))
        .arg("-I")
        .arg(root.join("runtime/cpp/include"))
        .arg(root.join(source))
        .arg("-L")
        .arg(&release)
        .args(["-lzerodds", "-o"])
        .arg(&bin)
        .output()
        .expect("c++ compile");
    assert!(
        compiled.status.success(),
        "c++ compile failed:\n{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let lease = zerodds_test_domains::lease_domain();
    let run = Command::new(&bin)
        .arg(lease.id().to_string())
        .env(
            if cfg!(target_os = "macos") {
                "DYLD_LIBRARY_PATH"
            } else {
                "LD_LIBRARY_PATH"
            },
            &release,
        )
        .output()
        .expect("run");
    let _ = std::fs::remove_file(&bin);
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success() && stdout.contains(ok_line),
        "the program failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
}
