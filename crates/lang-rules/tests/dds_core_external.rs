// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `dds::core::external<T>` of DDS-XTypes 1.3 §7.5.1.2.3.3, compiled and
//! run: `runtime/cpp/tests/external_selftest.cpp` checks every operation of
//! §7.5.1.2.3.3.1 to §7.5.1.2.3.3.15 and each row of Table 46.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::PathBuf;
use std::process::Command;

#[test]
fn the_template_does_what_every_operation_and_every_row_of_table_46_says() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../runtime/cpp");
    let dir = std::env::temp_dir().join(format!("zd-core-external-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a work directory");
    let binary = dir.join("selftest");
    let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".to_string());
    let compiled = Command::new(&cxx)
        .args(["-std=c++17", "-Wall", "-Wextra", "-Wpedantic", "-Werror"])
        .arg(format!("-I{}", root.join("include").display()))
        .arg(root.join("tests/external_selftest.cpp"))
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("the C++ compiler starts");
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let ran = Command::new(&binary).output().expect("the program starts");
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stdout)
    );
    let _ = std::fs::remove_dir_all(&dir);
}
