// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A generated type through the DDS-PSM-Cxx DataWriter<T> and DataReader<T>.
//!
//! `cpp_smoke_via_cargo.rs` exercises the PSM with the hand-written
//! `ByteSeq` specialization only, whose two-argument `decode` and missing
//! wire contract hid three defects of the typed path: `DataReader<T>::take()`
//! did not compile against the four-argument `decode` `zerodds-idlc generate
//! --lang c++` emits, the endpoints declared no TypeObject, extensibility or
//! representation, and a keyed sample was written without its key.
//!
//! The header is generated here from `tests/generated/typed_probe.idl` on
//! every run, so nothing generated is committed to go stale.
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

/// Makes `runtime/cpp/build/libzerodds_wire.a` once per test process. The
/// three tests run in parallel, and two `make` runs of the archive in the same
/// directory let `ar` read a half-written archive ("file format not
/// recognized") or find none.
fn make_runtime_cpp_archive(root: &Path) {
    static MADE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    MADE.get_or_init(|| {
        let mut make = Command::new("make");
        make.args(["-s", "-C"])
            .arg(root.join("runtime/cpp"))
            .arg("build/libzerodds_wire.a");
        run(make, "make runtime/cpp");
    });
}

#[test]
fn a_generated_type_round_trips_through_the_dds_psm_writer_and_reader() {
    let root = workspace_root();
    // The target directory `cpp_smoke_via_cargo.rs` builds into, for the
    // reason it gives: a build inside `cargo test` must not write where the
    // outer run reads.
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

    let work = std::env::temp_dir().join(format!("zerodds_cpp_generated_{}", std::process::id()));
    std::fs::create_dir_all(&work).expect("work directory");
    let mut idlc = Command::new(release.join("zerodds-idlc"));
    idlc.args([
        "generate",
        "--lang",
        "c++",
        "--cpp-mapping",
        "native",
        "--out",
    ])
    .arg(&work)
    .arg(root.join("crates/cpp/tests/generated/typed_probe.idl"));
    run(idlc, "zerodds-idlc generate --lang c++");

    make_runtime_cpp_archive(&root);

    let bin = work.join("generated_type_roundtrip");
    let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".to_string());
    let mut compile = Command::new(&cxx);
    compile
        .args(["-std=c++17", "-Wall", "-Werror", "-I"])
        .arg(&work)
        .arg("-I")
        .arg(root.join("crates/cpp/include"))
        .arg("-I")
        .arg(root.join("crates/zerodds-c-api/include"))
        .arg("-I")
        .arg(root.join("runtime/cpp/include"))
        .arg(root.join("crates/cpp/tests/generated_type_roundtrip.cpp"))
        .arg(root.join("runtime/cpp/build/libzerodds_wire.a"))
        .arg("-L")
        .arg(&release)
        .args(["-lzerodds", "-o"])
        .arg(&bin);
    run(compile, "c++ compile");

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
    let out = run(program, "generated_type_roundtrip");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("GENERATED TYPE ROUNDTRIP OK"),
        "the roundtrip did not report success:\n{stdout}"
    );
    let _ = std::fs::remove_dir_all(&work);
}

/// The same, for the DDS-PSM-Cxx 1.0 §8.1 mapping of `--cpp-mapping psm`: classes with
/// accessors, `dds::core::optional`, `std::map`, `std::wstring`, `long double`, handed to the
/// same codec through `zerodds::psm::Conv`.
#[test]
fn a_psm_mapped_type_round_trips_through_the_dds_psm_writer_and_reader() {
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

    let work = std::env::temp_dir().join(format!("zerodds_cpp_psm_{}", std::process::id()));
    std::fs::create_dir_all(&work).expect("work directory");
    let mut idlc = Command::new(release.join("zerodds-idlc"));
    idlc.args(["generate", "--lang", "c++", "--cpp-mapping", "psm", "--out"])
        .arg(&work)
        .arg(root.join("crates/cpp/tests/generated/psm_probe.idl"));
    run(idlc, "zerodds-idlc generate --lang c++ --cpp-mapping psm");

    make_runtime_cpp_archive(&root);

    let bin = work.join("generated_psm_roundtrip");
    let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".to_string());
    let mut compile = Command::new(&cxx);
    compile
        .args(["-std=c++17", "-Wall", "-Werror", "-I"])
        .arg(&work)
        .arg("-I")
        .arg(root.join("crates/cpp/include"))
        .arg("-I")
        .arg(root.join("crates/zerodds-c-api/include"))
        .arg("-I")
        .arg(root.join("runtime/cpp/include"))
        .arg(root.join("crates/cpp/tests/generated_psm_roundtrip.cpp"))
        .arg(root.join("runtime/cpp/build/libzerodds_wire.a"))
        .arg("-L")
        .arg(&release)
        .args(["-lzerodds", "-o"])
        .arg(&bin);
    run(compile, "c++ compile");

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
    let out = run(program, "generated_psm_roundtrip");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("GENERATED PSM ROUNDTRIP OK"),
        "the roundtrip did not report success:\n{stdout}"
    );
    let _ = std::fs::remove_dir_all(&work);
}

/// A MultiTopic over generated types (DDS 1.4 Annex B.2): the topics `Order` and `Customer`,
/// the MultiTopic `Joined` of the type `Sale`, and a `DataReader<Sale>` on it.
#[test]
fn a_multitopic_joins_two_generated_topics_for_a_data_reader() {
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

    let work = std::env::temp_dir().join(format!("zerodds_cpp_multitopic_{}", std::process::id()));
    std::fs::create_dir_all(&work).expect("work directory");
    let mut idlc = Command::new(release.join("zerodds-idlc"));
    idlc.args([
        "generate",
        "--lang",
        "c++",
        "--cpp-mapping",
        "native",
        "--out",
    ])
    .arg(&work)
    .arg(root.join("crates/cpp/tests/generated/multitopic_probe.idl"));
    run(idlc, "zerodds-idlc generate --lang c++");

    make_runtime_cpp_archive(&root);

    let bin = work.join("generated_multitopic");
    let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".to_string());
    let mut compile = Command::new(&cxx);
    compile
        .args(["-std=c++17", "-Wall", "-Werror", "-I"])
        .arg(&work)
        .arg("-I")
        .arg(root.join("crates/cpp/include"))
        .arg("-I")
        .arg(root.join("crates/zerodds-c-api/include"))
        .arg("-I")
        .arg(root.join("runtime/cpp/include"))
        .arg(root.join("crates/cpp/tests/generated_multitopic.cpp"))
        .arg(root.join("runtime/cpp/build/libzerodds_wire.a"))
        .arg("-L")
        .arg(&release)
        .args(["-lzerodds", "-o"])
        .arg(&bin);
    run(compile, "c++ compile");

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
    let out = run(program, "generated_multitopic");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("GENERATED MULTITOPIC OK"),
        "the multitopic test did not report success:\n{stdout}"
    );
    let _ = std::fs::remove_dir_all(&work);
}
