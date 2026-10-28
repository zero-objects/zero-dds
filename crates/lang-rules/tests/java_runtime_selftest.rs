// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The self-test of the Java wire runtime (`runtime/java/run-tests.sh`), compiled under
//! `-Xlint:all -Werror` and run.
//!
//! It holds, among the wire rules, that an `octet` is a Java `byte` the writer writes as the
//! number its eight bits are (CORBA 3.3 part 3 §1.5.2).
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

use std::path::Path;
use std::process::Command;

#[test]
fn the_java_runtime_selftest_passes() {
    let have_jdk = ["javac", "java"].iter().all(|candidate| {
        Command::new(candidate)
            .arg("-version")
            .output()
            .is_ok_and(|out| out.status.success())
    });
    if !have_jdk {
        eprintln!("SKIPPED: `javac` or `java` is not usable.");
        return;
    }
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime/java/run-tests.sh");
    let output = Command::new("bash")
        .arg(&script)
        .output()
        .expect("run-tests.sh runs");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "runtime/java/run-tests.sh:\n{text}"
    );
}
