// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The self-test of the C++ wire runtime (`runtime/cpp/tests/selftest.cpp`), built under the
//! warning set of its Makefile and run.
//!
//! It holds, among the wire rules, the type rules of DDS-XTypes 1.3 §7.5.1.1.2: every type of
//! Table 43 but `Char16` is told from every other by an overload, `Byte` from `UInt8`
//! included.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

use std::path::Path;
use std::process::Command;

#[test]
fn the_cpp_runtime_selftest_passes() {
    let have_compiler = ["c++", "g++", "clang++"].iter().any(|candidate| {
        Command::new(candidate)
            .arg("--version")
            .output()
            .is_ok_and(|out| out.status.success())
    });
    if !have_compiler {
        eprintln!("SKIPPED: no C++ compiler on the path.");
        return;
    }
    let runtime = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime/cpp");
    let output = Command::new("make")
        .arg("-C")
        .arg(&runtime)
        .arg("test")
        .output()
        .expect("make runs");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "make -C runtime/cpp test:\n{text}");
}
