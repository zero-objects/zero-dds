// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C TypeSupport `zerodds-idlc generate --lang c` writes into
//! `<stem>_typesupport.h`, compiled and run.
//!
//! `tests/c-typesupport/harness.c` puts one sample of `t::Reading` —
//! every shape the walks over a decoded sample follow — through the
//! table: the size probe and the encode, a decode of the codec header's
//! own octets in both byte orders, the decoded sample written again and
//! compared octet for octet, a truncated buffer refused, every decoded
//! sample released. What it prints is held against the wire probe's
//! reflective encoder over the same sample (`tests/c-typesupport/
//! probe.json`): the table's XCDR2 octets and its KeyHash.
//!
//! Built with AddressSanitizer where the compiler has it: a walk that
//! released storage it does not own, or kept storage it does, fails the
//! run with the sanitizer's report. On Linux that includes leaks.
//!
//! Twice: the codec header generated for XCDR2, whose functions the
//! table writes and reads XCDR2 through, and for XCDR1, where the table
//! still writes XCDR2 — the C API's writer stamps it — through a codec
//! of its own and reads the header's XCDR1.
//!
//! Without a C compiler the test reports the skip and passes.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_ast_layout::key::Purpose;
use zerodds_ast_layout::{Dialect, Encoding, Endianness, LayoutKey};
use zerodds_wire_probe::encode::{encode, hex, model_from_idl_in_dialect};
use zerodds_wire_probe::value::{Emit, MapLengthPolicy, ProbeRequest};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("c-typesupport")
}

fn work(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "zd-idlc-c-typesupport-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ))
}

fn cc_available() -> bool {
    Command::new("cc")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// `-fsanitize=address` when the compiler builds and runs a program with
/// it, nothing otherwise.
fn sanitizer(dir: &Path) -> Vec<&'static str> {
    let source = dir.join("asan_probe.c");
    let binary = dir.join("asan_probe");
    std::fs::write(&source, "int main(void) { return 0; }\n").expect("the probe source");
    let flags = vec!["-fsanitize=address", "-fno-omit-frame-pointer"];
    let built = Command::new("cc")
        .args(&flags)
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .is_ok_and(|output| output.status.success());
    if built
        && Command::new(&binary)
            .output()
            .is_ok_and(|output| output.status.success())
    {
        flags
    } else {
        eprintln!("NOTE: AddressSanitizer is not usable here; the run checks values only.");
        Vec::new()
    }
}

/// What the wire probe writes for the sample: its XCDR2 little-endian
/// octets and its KeyHash.
fn expected() -> (String, String) {
    let idl = std::fs::read_to_string(fixtures().join("probe.idl")).expect("the IDL");
    let values: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(fixtures().join("probe.json")).expect("the values"),
    )
    .expect("the values parse");
    let ast =
        model_from_idl_in_dialect(&idl, "probe.idl", Dialect::Specification).expect("the model");
    let key = LayoutKey::in_dialect(
        Dialect::Specification,
        Encoding::PlainCdr2,
        Endianness::Little,
        Purpose::Data,
    );
    let request = ProbeRequest::from_key("t::Reading", key, MapLengthPolicy::SequenceEquivalent);
    let payload = encode(&ast, &request, &values).expect("the payload");
    let key_hash = encode(&ast, &request.emitting(Emit::KeyHash), &values).expect("the KeyHash");
    (hex(&payload), hex(&key_hash))
}

/// Generate for `encoding`, build the harness against the result and run
/// it; its standard output.
fn build_and_run(tag: &str, encoding: &str, main_repr: &str, main_align: &str) -> String {
    let dir = work(tag);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the scratch directory");
    let generate = Command::new(env!("CARGO_BIN_EXE_zerodds-idlc"))
        .args(["generate", "--lang", "c", "--encoding", encoding, "--out"])
        .arg(&dir)
        .arg(fixtures().join("probe.idl"))
        .output()
        .expect("idlc runs");
    assert!(
        generate.status.success(),
        "generate --lang c --encoding {encoding}:\n{}",
        String::from_utf8_lossy(&generate.stderr)
    );
    assert!(dir.join("probe_typesupport.h").is_file(), "no companion");

    let root = repository_root();
    let runtime = root.join("runtime/c");
    let binary = dir.join("harness");
    let sanitizer = sanitizer(&dir);
    let output = Command::new("cc")
        .args([
            "-std=c99",
            "-O1",
            "-g",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Wconversion",
            "-Wsign-conversion",
            "-Werror",
        ])
        .args(&sanitizer)
        .arg(format!("-DMAIN_REPR={main_repr}"))
        .arg(format!("-DMAIN_ALIGN={main_align}"))
        .arg("-I")
        .arg(&dir)
        .arg("-I")
        .arg(runtime.join("include"))
        .arg("-I")
        .arg(root.join("crates/zerodds-c-api/include"))
        .arg(fixtures().join("harness.c"))
        .arg(runtime.join("src/writer.c"))
        .arg(runtime.join("src/reader.c"))
        .arg(runtime.join("src/md5.c"))
        .arg(runtime.join("src/float128.c"))
        .arg(runtime.join("src/float128_decimal.c"))
        .args(["-lm", "-o"])
        .arg(&binary)
        .output()
        .expect("cc runs");
    assert!(
        output.status.success(),
        "the harness did not compile against the {encoding} companion:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let run = Command::new(&binary)
        .env("ASAN_OPTIONS", "detect_leaks=1:abort_on_error=0")
        .output()
        .expect("the harness runs");
    assert!(
        run.status.success(),
        "the harness failed under the {encoding} companion: {:?}\n{}\n{}",
        run.status.code(),
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
    String::from_utf8_lossy(&run.stdout).into_owned()
}

fn field<'a>(stdout: &'a str, name: &str) -> &'a str {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix(name).map(str::trim))
        .unwrap_or_else(|| panic!("no `{name}` line in:\n{stdout}"))
}

#[test]
fn the_table_writes_reads_and_releases_a_sample() {
    if !cc_available() {
        eprintln!("SKIPPED: `cc` is not usable.");
        return;
    }
    let stdout = build_and_run("xcdr2", "xcdr2", "ZERODDS_TS_REPR_XCDR2", "4u");
    assert!(stdout.lines().any(|line| line == "OK"), "{stdout}");
    let (payload, key_hash) = expected();
    assert_eq!(field(&stdout, "xcdr2 "), payload, "the table's octets");
    assert_eq!(field(&stdout, "keyhash "), key_hash, "the table's KeyHash");
}

#[test]
fn generated_for_xcdr1_the_table_still_writes_xcdr2() {
    if !cc_available() {
        eprintln!("SKIPPED: `cc` is not usable.");
        return;
    }
    let stdout = build_and_run("xcdr1", "xcdr1", "ZERODDS_TS_REPR_XCDR1", "8u");
    assert!(stdout.lines().any(|line| line == "OK"), "{stdout}");
    let (payload, key_hash) = expected();
    assert_eq!(field(&stdout, "xcdr2 "), payload, "the table's octets");
    assert_eq!(field(&stdout, "keyhash "), key_hash, "the table's KeyHash");
}
