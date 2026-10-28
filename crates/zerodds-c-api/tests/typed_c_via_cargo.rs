// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A generated type through the C API's typed path.
//!
//! `zerodds-idlc generate --lang c` writes `<stem>_typesupport.h`, one
//! `zerodds_typesupport_t` per topic type. `tests/typed_c_roundtrip.c`
//! writes three keyed samples of `tests/generated/typed_c_probe.idl`
//! through `zerodds_writer_write_typed` and takes them back through
//! `zerodds_reader_take_typed` in one runtime, compares each with what was
//! written and releases it with the table's `sample_free`.
//!
//! The header is generated on every run, so nothing generated is committed
//! to go stale. Built into the target directory the C++ tests use, for the
//! reason `crates/cpp/tests/cpp_smoke_via_cargo.rs` gives.
#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr, missing_docs)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(PathBuf::from)
        .expect("workspace root")
}

/// Runs `command`, panicking with both streams when it fails.
fn run(mut command: Command, what: &str) -> std::process::Output {
    let out = command.output().unwrap_or_else(|e| panic!("{what}: {e}"));
    assert!(
        out.status.success(),
        "{what} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

#[test]
fn a_generated_type_round_trips_through_the_typed_c_path() {
    let root = workspace_root();
    let build_dir = root.join("target/cpp-smoke");
    let mut cargo = Command::new(env!("CARGO"));
    cargo
        .args([
            "build",
            "-p",
            "zerodds-c-api",
            "-p",
            "zerodds-idlc",
            "--release",
        ])
        .arg("--target-dir")
        .arg(&build_dir)
        .current_dir(&root);
    run(cargo, "cargo build zerodds-c-api zerodds-idlc");
    let release = build_dir.join("release");

    let work = std::env::temp_dir().join(format!("zerodds_typed_c_{}", std::process::id()));
    std::fs::create_dir_all(&work).expect("work directory");
    let mut idlc = Command::new(release.join("zerodds-idlc"));
    idlc.args(["generate", "--lang", "c", "--out"])
        .arg(&work)
        .arg(root.join("crates/zerodds-c-api/tests/generated/typed_c_probe.idl"));
    run(idlc, "zerodds-idlc generate --lang c");

    let runtime = root.join("runtime/c");
    let bin = work.join("typed_c_roundtrip");
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".to_string());
    let mut compile = Command::new(&cc);
    compile
        .args(["-std=c99", "-Wall", "-Wextra", "-Werror", "-I"])
        .arg(&work)
        .arg("-I")
        .arg(runtime.join("include"))
        .arg("-I")
        .arg(root.join("crates/zerodds-c-api/include"))
        .arg(root.join("crates/zerodds-c-api/tests/typed_c_roundtrip.c"))
        .arg(runtime.join("src/writer.c"))
        .arg(runtime.join("src/reader.c"))
        .arg(runtime.join("src/md5.c"))
        .arg(runtime.join("src/float128.c"))
        .arg(runtime.join("src/float128_decimal.c"))
        .arg("-L")
        .arg(&release)
        .args(["-lzerodds", "-lm", "-o"])
        .arg(&bin);
    run(compile, "cc compile");

    let lease = zerodds_test_domains::lease_domain();
    let mut program = Command::new(&bin);
    program.arg(lease.id().to_string()).env(
        if cfg!(target_os = "macos") {
            "DYLD_LIBRARY_PATH"
        } else {
            "LD_LIBRARY_PATH"
        },
        &release,
    );
    let out = run(program, "typed_c_roundtrip");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("TYPED C ROUNDTRIP OK"),
        "the roundtrip did not report success:\n{stdout}"
    );
    let _ = std::fs::remove_dir_all(&work);
}
